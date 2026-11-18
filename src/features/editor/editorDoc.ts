import type { Draft } from "../../generated/Draft";
import { changedLines } from "./changedLines";

/** What the file holds on disk. `hash` is null when there is no file. */
export type Disk = { text: string; hash: string | null };

/**
 * What a tab knows about its file. Every text here has `\n` line endings; the caller
 * normalises. `text` is the editor's text as last reported, `sent` the text a save in
 * flight sent, and `line` the status line's copy.
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
  | { type: "keepMine" }
  | { type: "takeDisk" }
  | { type: "unsave" }
  | { type: "unsaved"; hash: string }
  | { type: "unsaveRefused" }
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
  const next = { ...edit(s, text), marks: [], undoOffered: false };
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
    if (d.text !== s.base.text) {
      // The base text is stale (an undo put the file back): learn it from the disk.
      if (!s.dirty) return reload(cleared, d, null);
      base = d;
    }
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
  };
  return said(state, `Saved ${name}.`, draftEffect(dirty));
}

/** One event applied to the state. Pure: the effects are for the caller to run. */
export function step(s: DocState, e: DocEvent, name: string): Out {
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
      return { state: s, effects: [{ type: "unsave", snapshot: s.lastSave.snapshot }] };
    case "unsaved":
      // The disk now holds the file as it was; the `disk` event that follows brings its text.
      return said({ ...s, lastSave: null, undoOffered: false, base: { text: s.base.text, hash: e.hash } }, `${name} is back as it was before the save.`, { type: "refetch" });
    case "unsaveRefused":
      return said({ ...s, lastSave: null, undoOffered: false }, `${name} changed after the save, so the save can't be undone.`);
    case "left":
      return unchanged({ ...s, marks: [], undoOffered: false });
  }
}
