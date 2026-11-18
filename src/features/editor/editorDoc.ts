import type { Draft } from "../../generated/Draft";
import { changedLines } from "./changedLines";

/** What the file holds on disk. `hash` is null when there is no file. */
export type Disk = { text: string; hash: string | null };

/**
 * What a tab knows about its file. Every text here has `\n` line endings; the caller
 * normalises. `text` is the editor's text as last reported, `sent` the text a save in
 * flight sent, and `line` the status line's copy. `saving` is true while a save or an
 * undo of one is in flight. `replaced` is what the disk held before the tab's last write
 * (`hash` null for no file), until the disk has been read again: a read that began before
 * the write can still arrive, and it is not somebody else's change.
 */
export type DocState = {
  base: Disk;
  text: string;
  dirty: boolean;
  conflict: Disk | null;
  deleted: boolean;
  saving: boolean;
  lastSave: { snapshot: string; hash: string } | null;
  marks: number[];
  line: string | null;
  undoOffered: boolean;
  sent: string | null;
  replaced: { hash: string | null } | null;
};

export type DocEvent =
  | { type: "opened"; disk: Disk; draft: Draft | null }
  | { type: "edited"; text: string }
  | { type: "disk"; disk: Disk }
  | { type: "gone" }
  | { type: "save" }
  | { type: "saved"; hash: string; snapshot: string | null }
  | { type: "saveConflict"; hash: string | null }
  | { type: "saveFailed"; line: string }
  | { type: "saveTooLarge" }
  | { type: "keepMine" }
  | { type: "takeDisk" }
  | { type: "unsave" }
  | { type: "unsaved"; hash: string }
  | { type: "unsaveRefused" }
  | { type: "unsaveFailed"; line: string }
  | { type: "left" };

/** What the caller runs. The draft effects use the state after the step. */
export type Effect =
  | { type: "setText"; text: string; marks: number[] }
  | { type: "putDraft" }
  | { type: "dropDraft" }
  | { type: "save"; expect: string | null }
  | { type: "unsave"; snapshot: string }
  | { type: "refetch" }
  | { type: "announce"; text: string };

type Out = { state: DocState; effects: Effect[] };

export const start: DocState = {
  base: { text: "", hash: null },
  text: "",
  dirty: false,
  conflict: null,
  deleted: false,
  saving: false,
  lastSave: null,
  marks: [],
  line: null,
  undoOffered: false,
  sent: null,
  replaced: null,
};

/** A file that is not there reads as empty text. */
const NO_FILE: Disk = { text: "", hash: null };

const lines = (text: string) => text.split("\n");
const draftEffect = (dirty: boolean): Effect => ({ type: dirty ? "putDraft" : "dropDraft" });
const unchanged = (state: DocState): Out => ({ state, effects: [] });

/** A line for the status line that is also announced. */
function said(state: DocState, text: string, ...more: Effect[]): Out {
  return { state: { ...state, line: text }, effects: [...more, { type: "announce", text }] };
}

/**
 * The state when the text is `text`. While a conflict is up the tab stays dirty unless
 * the text is what the disk holds, which settles the conflict.
 */
function edit(s: DocState, text: string): DocState {
  if (s.conflict !== null && text === s.conflict.text) {
    return { ...s, text, base: s.conflict, conflict: null, deleted: false, dirty: false };
  }
  return { ...s, text, dirty: text !== s.base.text || s.conflict !== null };
}

function opened(disk: Disk, draft: Draft | null): Out {
  const fresh: DocState = { ...start, marks: [], deleted: disk.hash === null };
  if (draft === null) return unchanged({ ...fresh, text: disk.text, base: disk });
  if (draft.text === disk.text) return { state: { ...fresh, text: disk.text, base: disk }, effects: [{ type: "dropDraft" }] };
  if (disk.hash === null) {
    // The file is gone: the draft's text stays, and a save makes the file again.
    return unchanged({ ...fresh, text: draft.text, base: NO_FILE, dirty: draft.text !== "" });
  }
  if (draft.base_hash === disk.hash) return unchanged({ ...fresh, text: draft.text, base: disk, dirty: true });
  return unchanged({ ...fresh, text: draft.text, base: { text: draft.base_text, hash: draft.base_hash }, conflict: disk, dirty: true });
}

function edited(s: DocState, text: string): Out {
  // The status line spoke of the text as it was ("Saved a.txt."), so it goes with the first change.
  const next = { ...edit(s, text), marks: [], undoOffered: false, line: null };
  return { state: next, effects: [draftEffect(next.dirty)] };
}

function gone(s: DocState): Out {
  if (s.deleted && s.base.hash === null && s.conflict === null) return unchanged(s);
  const dirty = s.text !== "";
  const state = { ...s, base: NO_FILE, conflict: null, deleted: true, dirty };
  if (dirty) return { state, effects: [{ type: "putDraft" }] };
  return { state, effects: s.dirty ? [{ type: "dropDraft" }] : [] };
}

/** Clean: the disk's text goes into the editor. */
function reload(s: DocState, disk: Disk, announce: string | null): Out {
  const marks = changedLines(lines(s.text), lines(disk.text));
  const state: DocState = { ...s, text: disk.text, base: disk, conflict: null, deleted: false, dirty: false, marks };
  const set: Effect = { type: "setText", text: disk.text, marks };
  if (announce === null) return { state, effects: [set] };
  const count = marks.length === 1 ? "1 line changed." : `${marks.length} lines changed.`;
  return said(state, `${announce} changed on disk. ${count}`, set);
}

function disk(s: DocState, d: Disk, name: string): Out {
  if (d.hash === null) return gone(s);
  if (d.hash === s.base.hash) {
    // Nobody else changed the file. A conflict that was up is over.
    const cleared: DocState = { ...s, conflict: null };
    let base = s.base;
    // The base text is stale (an undo put the file back): learn it from the disk. The editor keeps the person's text, which is
    // unsaved again (PLAN §9, 6q-4 decision 8).
    if (d.text !== s.base.text) base = d;
    const dirty = s.text !== base.text;
    if (base === s.base && s.conflict === null && dirty === s.dirty) return unchanged(s);
    return { state: { ...cleared, base, dirty }, effects: [draftEffect(dirty)] };
  }
  if (d.text === s.text) {
    // The disk already holds what the editor holds.
    return { state: { ...s, base: d, conflict: null, deleted: false, dirty: false }, effects: s.dirty ? [{ type: "dropDraft" }] : [] };
  }
  if (s.dirty) return unchanged({ ...s, conflict: d, deleted: false });
  return reload(s, d, name);
}

function save(s: DocState): Out {
  if (s.saving || s.conflict !== null || (!s.dirty && s.base.hash !== null)) return unchanged(s);
  return { state: { ...s, saving: true, sent: s.text }, effects: [{ type: "save", expect: s.base.hash }] };
}

function saved(s: DocState, hash: string, snapshot: string | null, name: string): Out {
  if (s.sent === null) return { state: s, effects: [{ type: "refetch" }] };
  const conflict = s.conflict !== null && s.conflict.hash === hash ? null : s.conflict;
  const dirty = s.text !== s.sent || conflict !== null;
  const state: DocState = {
    ...s,
    base: { text: s.sent, hash },
    conflict,
    deleted: false,
    saving: false,
    sent: null,
    dirty,
    lastSave: snapshot === null ? null : { snapshot, hash },
    undoOffered: snapshot !== null,
    replaced: s.base.hash === hash ? null : { hash: s.base.hash },
  };
  // The file is read again at once, so what the pane knows of it (its hash, that it is there) is the saved file.
  return said(state, `Saved ${name}.`, draftEffect(dirty), { type: "refetch" });
}

/**
 * The first read of the disk after a write of the tab's own. One that shows what the write replaced began before it: it is
 * dropped and the file is read again, so a file that really went back is taken on that second read.
 */
function afterOwnWrite(s: DocState, hash: string | null): Out | null {
  if (s.replaced === null) return null;
  const state = { ...s, replaced: null };
  if (hash === s.replaced.hash && hash !== s.base.hash) return { state, effects: [{ type: "refetch" }] };
  return null;
}

/** One event applied to the state. Pure: the effects are for the caller to run. */
export function step(s: DocState, e: DocEvent, name: string): Out {
  if (e.type === "disk" || e.type === "gone") {
    const stale = afterOwnWrite(s, e.type === "disk" ? e.disk.hash : null);
    if (stale) return stale;
    if (s.replaced !== null) s = { ...s, replaced: null };
  }
  switch (e.type) {
    case "opened":
      return opened(e.disk, e.draft);
    case "edited":
      return edited(s, e.text);
    case "disk":
      return disk(s, e.disk, name);
    case "gone":
      return gone(s);
    case "save":
      return save(s);
    case "saved":
      return saved(s, e.hash, e.snapshot, name);
    case "saveConflict":
      return said({ ...s, saving: false, sent: null }, `${name} changed on disk, so it wasn't saved.`, { type: "refetch" });
    case "saveFailed":
      return said({ ...s, saving: false, sent: null }, `${name} couldn't be saved. ${e.line}`);
    case "saveTooLarge":
      return said({ ...s, saving: false, sent: null }, `${name} is over 2 MB, so it can't be saved.`);
    case "keepMine": {
      if (s.conflict === null || s.saving) return unchanged(s);
      const next = edit({ ...s, base: s.conflict, conflict: null }, s.text);
      return { state: next, effects: [draftEffect(next.dirty)] };
    }
    case "takeDisk": {
      if (s.conflict === null || s.saving) return unchanged(s);
      const d = s.conflict;
      const state: DocState = { ...s, text: d.text, base: d, conflict: null, deleted: false, dirty: false, marks: [] };
      return { state, effects: [{ type: "setText", text: d.text, marks: [] }, { type: "dropDraft" }] };
    }
    case "unsave":
      if (s.lastSave === null || s.saving) return unchanged(s);
      // In flight like a save, so a second press, a save or the bar waits for its answer.
      return { state: { ...s, saving: true }, effects: [{ type: "unsave", snapshot: s.lastSave.snapshot }] };
    case "unsaved":
      // The disk now holds the file as it was; the `disk` event that follows brings its text.
      return said(
        { ...s, saving: false, lastSave: null, undoOffered: false, base: { text: s.base.text, hash: e.hash }, replaced: s.base.hash === e.hash ? null : { hash: s.base.hash } },
        `${name} is back as it was before the save.`,
        { type: "refetch" },
      );
    case "unsaveRefused":
      return said({ ...s, saving: false, lastSave: null, undoOffered: false }, `${name} changed after the save, so the save can't be undone.`);
    case "unsaveFailed":
      // The core would not write the file (read-only, in use): the save is still there to undo.
      return said({ ...s, saving: false }, `The save of ${name} couldn't be undone. ${e.line}`);
    case "left":
      return unchanged({ ...s, marks: [], undoOffered: false });
  }
}
