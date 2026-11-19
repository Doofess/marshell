import type { Draft } from "../../generated/Draft";
import { changedLines } from "./changedLines";
import { countConflicts } from "./conflicts";

/** What the file holds on disk. `hash` is null when there is no file. */
export type Disk = { text: string; hash: string | null };

/**
 * What a tab knows about its file. Every text here has `\n` line endings; the caller
 * normalises. `text` is the editor's text as last reported, `sent` the text a save in
 * flight sent, and `line` the status line's copy. `saving` is true while a save or an
 * undo of one is in flight. `replaced` is what the disk held before the tab's last write
 * (`hash` null for no file), until the disk has been read again: a read that began before
 * the write can still arrive, and it is not somebody else's change. `held` is the newest read of
 * the disk that came while a save or an undo was in flight: it is taken when that is answered.
 * `merging` is true while a merge request is out whose answer is wanted, which is only while the bar is up, and `mergeId` is the
 * number of the last request made: an answer with another number is for a request that was given up. `merge` is the last merge of the open tab: `mine` is the text before it, which
 * rests on the file `before`, and `merged` is the text after it, which rests on the file `after`. The bar follows whichever of the
 * two texts the editor holds, so an undo of the merge brings it back and a redo takes it away.
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
  held: Extract<DocEvent, { type: "disk" | "gone" }> | null;
  merging: boolean;
  mergeId: number;
  merge: { mine: string; merged: string; before: Disk; after: Disk } | null;
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
  | { type: "merge" }
  /** The answer to a merge: `mine` and `disk` are the text and the disk hash the request was made from. */
  | { type: "merged"; id: number; text: string; conflicts: number; mine: string; disk: string | null }
  | { type: "mergeFailed"; id: number; line: string }
  | { type: "left" };

/** What the caller runs. The draft effects use the state after the step. */
export type Effect =
  | { type: "setText"; text: string; marks: number[] }
  | { type: "putDraft" }
  | { type: "dropDraft" }
  | { type: "save"; expect: string | null }
  | { type: "unsave"; snapshot: string }
  | { type: "refetch" }
  | { type: "merge"; id: number; base: string; mine: string; disk: Disk }
  | { type: "showConflict" }
  | { type: "announce"; text: string; /** Only for someone whose keyboard is in the tab. */ reading?: true };

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
  held: null,
  merging: false,
  mergeId: 0,
  merge: null,
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

/**
 * The text the editor holds decides which file it rests on, where the last merge is concerned: the text from before the merge
 * rests on the file as it was loaded (the bar is up), and the merged text on the file as it was merged (the bar is down).
 */
function afterMerge(s: DocState, text: string): DocState {
  const m = s.merge;
  if (m === null) return s;
  if (text === m.mine && s.base.hash === m.after.hash) return { ...s, base: m.before, conflict: s.conflict ?? m.after };
  if (text === m.merged && s.base.hash === m.before.hash && s.conflict !== null) {
    return { ...s, base: m.after, conflict: s.conflict.hash === m.after.hash ? null : s.conflict };
  }
  return s;
}

const mergingLine = (name: string) => `Merging ${name}…`;

function edited(s: DocState, text: string, name: string): Out {
  // The status line spoke of the text as it was ("Saved a.txt."), so it goes with the first change. A merge that runs is still
  // running: its answer will replace what is typed now, so the line keeps saying so.
  const next = { ...edit(afterMerge(s, text), text), marks: [], undoOffered: false, line: s.merging ? mergingLine(name) : null };
  return { state: next, effects: [draftEffect(next.dirty)] };
}

/** Asks for a merge of what is there now, under a new number: an answer to an earlier request no longer counts. */
function asked(s: DocState, disk: Disk, name: string): Out {
  const id = s.mergeId + 1;
  return { state: { ...s, merging: true, mergeId: id, line: mergingLine(name) }, effects: [{ type: "merge", id, base: s.base.text, mine: s.text, disk }] };
}

function merge(s: DocState, name: string): Out {
  if (s.conflict === null || s.saving || s.merging) return unchanged(s);
  return asked(s, s.conflict, name);
}

function mergedLine(name: string, conflicts: number, own: boolean): string {
  if (own) return `Merged ${name}. No conflicts.`;
  if (conflicts === 0) return `Merged ${name}. No conflicts. Ctrl+Z undoes the merge.`;
  const count = conflicts === 1 ? "1 conflict" : `${conflicts} conflicts`;
  return `Merged ${name}. ${count} to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge.`;
}

/**
 * A merge is wanted only while the bar is up. However the bar went (it was answered, the file went, the disk came to hold the
 * text or went back to what was loaded, the person typed the disk's text or redid an earlier merge), the merge that ran is over:
 * its answer is dropped when it comes, and a bar that comes up later is not merged unasked.
 */
function settled(out: Out, name: string): Out {
  const s = out.state;
  if (!s.merging || s.conflict !== null) return out;
  return { state: { ...s, merging: false, line: s.line === mergingLine(name) ? null : s.line }, effects: out.effects };
}

/** An answer counts only for the request that is out now. */
const answers = (s: DocState, id: number) => s.merging && id === s.mergeId;

function merged(s: DocState, e: Extract<DocEvent, { type: "merged" }>, name: string): Out {
  if (!answers(s, e.id) || s.conflict === null) return unchanged(s);
  // The text or the file moved on: the answer is for something that is no longer there, so the merge runs again on what is.
  if (e.mine !== s.text || e.disk !== s.conflict.hash) return asked(s, s.conflict, name);
  const own = e.text === e.mine;
  const marks = changedLines(lines(e.mine), lines(e.text));
  const dirty = e.text !== s.conflict.text;
  const state: DocState = {
    ...s,
    text: e.text,
    base: s.conflict,
    conflict: null,
    deleted: false,
    dirty,
    marks,
    merging: false,
    merge: own ? null : { mine: e.mine, merged: e.text, before: s.base, after: s.conflict },
  };
  const more: Effect[] = [{ type: "setText", text: e.text, marks }];
  if (e.conflicts > 0) more.push({ type: "showConflict" });
  more.push(draftEffect(dirty));
  return said(state, mergedLine(name, e.conflicts, own), ...more);
}

function gone(s: DocState): Out {
  if (s.deleted && s.base.hash === null && s.conflict === null) return unchanged(s.merge !== null ? { ...s, merge: null } : s);
  const dirty = s.text !== "";
  const state = { ...s, base: NO_FILE, conflict: null, deleted: true, dirty, merge: null };
  if (dirty) return { state, effects: [{ type: "putDraft" }] };
  return { state, effects: s.dirty ? [{ type: "dropDraft" }] : [] };
}

/** Clean: the disk's text goes into the editor. */
function reload(s: DocState, disk: Disk, announce: string | null): Out {
  const marks = changedLines(lines(s.text), lines(disk.text));
  const state: DocState = { ...s, text: disk.text, base: disk, conflict: null, deleted: false, dirty: false, marks, merge: null };
  const set: Effect = { type: "setText", text: disk.text, marks };
  if (announce === null) return { state, effects: [set] };
  const count = marks.length === 1 ? "1 line changed." : `${marks.length} lines changed.`;
  const text = `${announce} changed on disk. ${count}`;
  // A file an agent writes reloads again and again: the line shows each time, and is spoken only to someone reading in the tab.
  return { state: { ...state, line: text }, effects: [set, { type: "announce", text, reading: true }] };
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
    return { state: { ...s, base: d, conflict: null, deleted: false, dirty: false, merge: null }, effects: s.dirty ? [{ type: "dropDraft" }] : [] };
  }
  if (s.dirty) return unchanged({ ...s, conflict: d, deleted: false });
  return reload(s, d, name);
}

function save(s: DocState, name: string): Out {
  if (s.saving) return unchanged(s);
  // The bar is up: nothing is written until it is answered, and the person who pressed the key is told so.
  if (s.conflict !== null) return said(s, `${name} changed on disk, so it wasn't saved.`);
  if (!s.dirty && s.base.hash !== null) return unchanged(s);
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
    merge: null,
  };
  // A text that still holds conflicts is saved as it is, and the line says how many are left.
  const left = countConflicts(s.sent);
  const rest = left === 0 ? "" : left === 1 ? " 1 conflict left." : ` ${left} conflicts left.`;
  // The file is read again at once, so what the pane knows of it (its hash, that it is there) is the saved file.
  return said(state, `Saved ${name}.${rest}`, draftEffect(dirty), { type: "refetch" });
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

/**
 * One event applied to the state. Pure: the effects are for the caller to run. A read of the disk that comes while a save or an
 * undo is in flight waits for the answer: before it, the read can't be told from the tab's own write, and taking it would reload
 * the text an undo is meant to leave (PLAN §9, 6q-4 decision 8) or raise the bar over the tab's own save.
 */
export function step(s: DocState, e: DocEvent, name: string): Out {
  if ((e.type === "disk" || e.type === "gone") && s.saving) return unchanged({ ...s, held: e });
  const out = settled(apply(s, e, name), name);
  const held = out.state.held;
  if (held === null || out.state.saving) return out;
  const next = settled(apply({ ...out.state, held: null }, held, name), name);
  return { state: next.state, effects: [...out.effects, ...next.effects] };
}

function apply(s: DocState, e: DocEvent, name: string): Out {
  if (e.type === "disk" || e.type === "gone") {
    const stale = afterOwnWrite(s, e.type === "disk" ? e.disk.hash : null);
    if (stale) return stale;
    if (s.replaced !== null) s = { ...s, replaced: null };
  }
  switch (e.type) {
    case "opened":
      return opened(e.disk, e.draft);
    case "edited":
      return edited(s, e.text, name);
    case "disk":
      return disk(s, e.disk, name);
    case "gone":
      return gone(s);
    case "save":
      return save(s, name);
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
      const next = edit({ ...s, base: s.conflict, conflict: null, merge: null }, s.text);
      return { state: next, effects: [draftEffect(next.dirty)] };
    }
    case "takeDisk": {
      if (s.conflict === null || s.saving) return unchanged(s);
      const d = s.conflict;
      const state: DocState = { ...s, text: d.text, base: d, conflict: null, deleted: false, dirty: false, marks: [], merge: null };
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
    case "merge":
      return merge(s, name);
    case "merged":
      return merged(s, e, name);
    case "mergeFailed":
      if (!answers(s, e.id)) return unchanged(s);
      return said({ ...s, merging: false }, `${name} couldn't be merged. ${e.line}`);
    case "left":
      return unchanged({ ...s, marks: [], undoOffered: false });
  }
}
