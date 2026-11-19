import type { FileSaved } from "../../../../generated/FileSaved";

/**
 * What a PDF tab knows about the save state of its notes. The notes themselves live in pdf.js; this is the state around them, the
 * way `editorDoc.ts` is for text. `base` is the file the notes rest on: its SHA-256 (null for no file) and its bytes. `dirty` is true
 * while there are notes that aren't in the file. `saving` is true while a save, an overwrite or an undo of a save is in flight.
 * `lastSave` is the save Undo can take back, and `hash` the file it wrote. `line` is the save line's copy, and `refusedSave` is
 * true while it is a refusal, which offers "Save as a copy". `conflict` is the file as the disk holds it when the bar is up: `bytes`
 * is null for a deleted file, and while the bytes of a changed file haven't been read yet. `fromDraft` is true while the notes
 * shown came from a draft.
 *
 * A read of the disk that comes while a save is in flight is left alone: the caller keeps the newest one and sends it again
 * once the save is answered, which is when it can tell the tab's own write from somebody else's.
 */
export type NotesState = {
  base: { hash: string | null; bytes: ArrayBuffer };
  dirty: boolean;
  saving: boolean;
  lastSave: { snapshot: string; hash: string } | null;
  line: string | null;
  refusedSave: boolean;
  conflict: { hash: string | null; bytes: ArrayBuffer | null } | null;
  fromDraft: boolean;
};

export type NotesEvent =
  /** The notes changed. The caller waits for a quiet spell before it runs the draft the effect asks for. */
  | { type: "edited" }
  | { type: "save" }
  /** The answer to a `serialize` effect: the PDF with the notes in it. */
  // `later`: a change came after pdf.js took the notes, so it isn't in these bytes.
  | { type: "serialized"; bytes: Uint8Array; for: "save" | "draft" | "copy" | "overwrite"; later?: boolean }
  /** The core wrote the file: `bytes` is what it was given. */
  | { type: "saved"; saved: FileSaved; bytes: Uint8Array }
  /** The core refused a save because the file isn't what the notes rest on. `hash` is null for a file that is gone. */
  | { type: "saveConflict"; hash: string | null }
  | { type: "saveRefused"; line: string }
  | { type: "unsave" }
  | { type: "unsaved"; hash: string }
  | { type: "unsaveRefused" }
  /** The core wouldn't write the file back (read-only, in use, unreachable): the save is still there to undo. */
  | { type: "unsaveFailed"; line: string }
  /** The file as the disk holds it now: `hash` and `bytes` are null for a file that is gone. */
  | { type: "disk"; hash: string | null; bytes: ArrayBuffer | null }
  /** The draft the core kept, when the tab opens. */
  | { type: "draft"; bytes: ArrayBuffer; baseHash: string | null }
  | { type: "copy" }
  | { type: "copied"; name: string }
  | { type: "reload" }
  | { type: "overwrite" }
  | { type: "draftRefused"; line: string };

/** What the caller runs. `putDraft`, `save` and `saveCopy` use the bytes of the `serialized` event that led to them. */
export type NotesEffect =
  | { type: "serialize"; then: "save" | "draft" | "copy" | "overwrite" }
  | { type: "save"; bytes: Uint8Array; expect: string | null }
  | { type: "saveCopy"; bytes: Uint8Array }
  | { type: "unsave"; snapshot: string }
  | { type: "putDraft" }
  | { type: "dropDraft" }
  /** Show these bytes in the tab. `dirty`: they hold notes that aren't in the file. */
  | { type: "load"; bytes: ArrayBuffer; dirty: boolean }
  | { type: "markSaved" }
  | { type: "announce"; text: string };

type Out = { state: NotesState; effects: NotesEffect[] };

const unchanged = (state: NotesState): Out => ({ state, effects: [] });

/** A line for the save line that is also announced. */
function said(state: NotesState, text: string, ...more: NotesEffect[]): Out {
  return { state: { ...state, line: text }, effects: [...more, { type: "announce", text }] };
}

const barLine = (name: string, hash: string | null) => (hash === null ? `${name} was deleted. Your notes aren't saved.` : `${name} changed on disk. Your notes aren't saved.`);

/** Asks for a draft of the notes as they are now. */
const DRAFT: NotesEffect = { type: "serialize", then: "draft" };

/** The bytes as an `ArrayBuffer`, without a copy when they fill their buffer. */
function toBuffer(b: Uint8Array): ArrayBuffer {
  if (b.byteOffset === 0 && b.byteLength === b.buffer.byteLength && b.buffer instanceof ArrayBuffer) return b.buffer;
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer;
}

function save(s: NotesState): Out {
  if (s.saving || s.conflict !== null) return unchanged(s);
  if (!s.dirty && s.base.hash !== null) return unchanged(s);
  return { state: { ...s, saving: true, refusedSave: false }, effects: [{ type: "serialize", then: "save" }] };
}

function serialized(s: NotesState, e: Extract<NotesEvent, { type: "serialized" }>): Out {
  switch (e.for) {
    case "save":
      if (!s.saving) return unchanged(s);
      // What is sent is what the file will hold: a change made since pdf.js took the notes is not in it, and keeps the tab dirty.
      return { state: { ...s, dirty: e.later === true }, effects: [{ type: "save", bytes: e.bytes, expect: s.base.hash }] };
    case "overwrite":
      if (!s.saving || s.conflict === null) return unchanged(s);
      return { state: { ...s, dirty: e.later === true }, effects: [{ type: "save", bytes: e.bytes, expect: s.conflict.hash }] };
    case "draft":
      // A save in flight decides what the draft is: its answer asks for a new one when the notes still aren't in the file.
      if (s.saving) return unchanged(s);
      return { state: s, effects: [{ type: s.dirty ? "putDraft" : "dropDraft" }] };
    case "copy":
      return { state: s, effects: [{ type: "saveCopy", bytes: e.bytes }] };
  }
}

function saved(s: NotesState, e: Extract<NotesEvent, { type: "saved" }>, name: string): Out {
  const state: NotesState = {
    ...s,
    base: { hash: e.saved.hash, bytes: toBuffer(e.bytes) },
    saving: false,
    conflict: null,
    refusedSave: false,
    fromDraft: false,
    lastSave: e.saved.snapshot === null ? null : { snapshot: e.saved.snapshot, hash: e.saved.hash },
  };
  // Notes made while the file was being written aren't in it: the draft goes on, and pdf.js keeps counting them as unsaved.
  const rest: NotesEffect[] = state.dirty ? [{ type: "serialize", then: "draft" }] : [{ type: "markSaved" }, { type: "dropDraft" }];
  return said(state, `Saved ${name}.`, ...rest);
}

function disk(s: NotesState, hash: string | null, bytes: ArrayBuffer | null, name: string): Out {
  if (s.saving) return unchanged(s);
  if (hash === null) {
    // The file is gone. A clean tab goes on showing what it has, as the pane does.
    if (!s.dirty || (s.conflict !== null && s.conflict.hash === null)) return unchanged(s);
    return { state: { ...s, conflict: { hash: null, bytes: null } }, effects: [{ type: "announce", text: barLine(name, null) }] };
  }
  // The file the notes rest on, or the one the tab's own save wrote: nothing changed.
  if (hash === s.base.hash) return s.conflict === null ? unchanged(s) : unchanged({ ...s, conflict: null });
  if (hash === s.lastSave?.hash) return unchanged(s);
  if (s.conflict !== null && s.conflict.hash === hash) {
    // The bar is up for this file already; a read can only add the bytes.
    if (s.conflict.bytes !== null || bytes === null) return unchanged(s);
    return unchanged({ ...s, conflict: { hash, bytes } });
  }
  if (s.dirty) return { state: { ...s, conflict: { hash, bytes } }, effects: [{ type: "announce", text: barLine(name, hash) }] };
  if (bytes === null) return unchanged(s);
  return { state: { ...s, base: { hash, bytes }, lastSave: null, line: null, refusedSave: false, fromDraft: false }, effects: [{ type: "load", bytes, dirty: false }] };
}

function draft(s: NotesState, bytes: ArrayBuffer, baseHash: string | null, name: string): Out {
  // A draft is for a tab that has just opened: one with notes of its own keeps them.
  if (s.dirty || s.saving || s.conflict !== null) return unchanged(s);
  const load: NotesEffect = { type: "load", bytes, dirty: true };
  if (baseHash === s.base.hash) return { state: { ...s, dirty: true, fromDraft: true }, effects: [load] };
  // The file isn't the one the draft grew from. The draft's notes show and stay unsaved; the file as it is now waits in the bar.
  const state: NotesState = {
    ...s,
    base: { hash: baseHash, bytes },
    dirty: true,
    fromDraft: true,
    conflict: s.base.hash === null ? { hash: null, bytes: null } : { hash: s.base.hash, bytes: s.base.bytes },
  };
  return { state, effects: [load, { type: "announce", text: barLine(name, s.base.hash) }] };
}

function reload(s: NotesState): Out {
  if (s.saving || s.conflict === null || s.conflict.hash === null || s.conflict.bytes === null) return unchanged(s);
  const { hash, bytes } = s.conflict;
  const state: NotesState = { ...s, base: { hash, bytes }, conflict: null, dirty: false, lastSave: null, line: null, refusedSave: false, fromDraft: false };
  return { state, effects: [{ type: "load", bytes, dirty: false }, { type: "dropDraft" }] };
}

/**
 * One event applied to the state. Pure: the effects are for the caller to run, and `name` is the file's name, which the copy
 * names. The copy is in the plan's Global Constraints.
 */
export function notesReducer(s: NotesState, e: NotesEvent, name: string): Out {
  switch (e.type) {
    case "edited":
      return { state: { ...s, dirty: true, line: null, refusedSave: false }, effects: [{ type: "serialize", then: "draft" }] };
    case "save":
      return save(s);
    case "serialized":
      return serialized(s, e);
    case "saved":
      return saved(s, e, name);
    case "saveConflict": {
      const state = { ...s, saving: false, dirty: true, refusedSave: false, line: null, conflict: { hash: e.hash, bytes: null } };
      return { state, effects: [{ type: "announce", text: barLine(name, e.hash) }, DRAFT] };
    }
    case "saveRefused":
      return said({ ...s, saving: false, dirty: true, refusedSave: true }, e.line, DRAFT);
    case "unsave":
      if (s.lastSave === null || s.saving) return unchanged(s);
      return { state: { ...s, saving: true }, effects: [{ type: "unsave", snapshot: s.lastSave.snapshot }] };
    case "unsaved":
      // The file is as it was before the save, and the notes are still in the tab, so they are unsaved again (6q-4 decision 8).
      return said(
        { ...s, saving: false, dirty: true, lastSave: null, refusedSave: false, base: { hash: e.hash, bytes: s.base.bytes } },
        `${name} is back as it was before the save.`,
        DRAFT,
      );
    case "unsaveRefused":
      return said({ ...s, saving: false, lastSave: null, refusedSave: false }, `${name} changed after the save, so the save can't be undone.`);
    case "unsaveFailed":
      return said({ ...s, saving: false }, `The save of ${name} couldn't be undone. ${e.line}`);
    case "disk":
      return disk(s, e.hash, e.bytes, name);
    case "draft":
      return draft(s, e.bytes, e.baseHash, name);
    case "copy":
      if (s.saving) return unchanged(s);
      return { state: s, effects: [{ type: "serialize", then: "copy" }] };
    case "copied":
      return said({ ...s, refusedSave: false }, `Saved the notes in ${e.name}.`);
    case "reload":
      return reload(s);
    case "overwrite":
      if (s.saving || s.conflict === null) return unchanged(s);
      return { state: { ...s, saving: true, refusedSave: false }, effects: [{ type: "serialize", then: "overwrite" }] };
    case "draftRefused":
      // The notes stay in the tab, unsaved, and the line says why they aren't kept. A put answered after the notes were saved
      // (or reloaded away) has nothing left to keep.
      if (!s.dirty) return unchanged(s);
      return said(s, e.line);
  }
}

/**
 * Whether the tab has notes that may not be in the file: unsaved ones, or ones a save, an overwrite or an undo of a save is
 * still deciding. The dot and the close question go by this, as a text tab's do, which stay until a save is answered.
 */
export function unsavedNotes(s: NotesState): boolean {
  return s.dirty || s.saving;
}

/** `{stem} (notes).pdf`, then `(notes 2)` up to `(notes 99)`; null when every one of them is taken. */
export function copyName(stem: string, taken: (name: string) => boolean): string | null {
  for (let n = 1; n <= 99; n++) {
    const name = n === 1 ? `${stem} (notes).pdf` : `${stem} (notes ${n}).pdf`;
    if (!taken(name)) return name;
  }
  return null;
}

/** The characters Windows-1252 has beyond ASCII and Latin-1's range: 0x80 to 0x9F, but for the five it leaves empty. */
const CP1252_EXTRAS = new Set([..."€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ"]);

/**
 * True when every character of the text is in Windows-1252, which is what the font pdf.js writes free text in covers: other
 * text shows blank in Edge and Chrome. Newlines, returns and tabs count as safe.
 */
export function winAnsiSafe(text: string): boolean {
  for (const ch of text) {
    const c = ch.codePointAt(0)!;
    if (c === 0x09 || c === 0x0a || c === 0x0d || (c >= 0x20 && c <= 0x7e) || (c >= 0xa0 && c <= 0xff) || CP1252_EXTRAS.has(ch)) continue;
    return false;
  }
  return true;
}
