import { describe, expect, it } from "vitest";
import type { FileSaved } from "../../../../generated/FileSaved";
import { copyName, notesReducer, unsavedNotes, winAnsiSafe, type NotesEffect, type NotesEvent, type NotesState } from "./notesDoc";

const N = "report.pdf";
const H1 = "1".repeat(64);
const H2 = "2".repeat(64);
const H3 = "3".repeat(64);
const bytesOf = (...n: number[]) => new Uint8Array(n);
const buf = (...n: number[]) => bytesOf(...n).buffer as ArrayBuffer;

/** A clean state resting on the file at hash H1. */
function doc(over: Partial<NotesState> = {}): NotesState {
  return { base: { hash: H1, bytes: buf(1, 2, 3) }, dirty: false, saving: false, lastSave: null, line: null, refusedSave: false, conflict: null, fromDraft: false, ...over };
}
const dirty = (over: Partial<NotesState> = {}) => doc({ dirty: true, ...over });
const fileSaved = (over: Partial<FileSaved> = {}): FileSaved => ({ size: 4, modified_ms: 9, hash: H2, snapshot: "snap-1", ...over });

function run(s: NotesState, ...events: NotesEvent[]) {
  let state = s;
  let effects: NotesEffect[] = [];
  for (const e of events) {
    const out = notesReducer(state, e, N);
    state = out.state;
    effects = out.effects;
  }
  return { state, effects };
}
const types = (effects: NotesEffect[]) => effects.map((e) => e.type);
const announced = (effects: NotesEffect[]) => effects.filter((e): e is Extract<NotesEffect, { type: "announce" }> => e.type === "announce").map((e) => e.text);

describe("notesDoc editing", () => {
  it("notesDoc_edited_makes_it_dirty_and_asks_for_a_draft", () => {
    const { state, effects } = run(doc({ line: "Saved report.pdf." }), { type: "edited" });
    expect(state).toMatchObject({ dirty: true, line: null, refusedSave: false });
    expect(effects).toEqual([{ type: "serialize", then: "draft" }]);
  });
});

describe("notesDoc save", () => {
  it("notesDoc_save_serializes_then_saves_with_the_base_hash", () => {
    const a = run(dirty(), { type: "save" });
    expect(a.state.saving).toBe(true);
    expect(a.effects).toEqual([{ type: "serialize", then: "save" }]);
    const b = run(a.state, { type: "serialized", bytes: bytesOf(9), for: "save" });
    expect(b.effects).toEqual([{ type: "save", bytes: bytesOf(9), expect: H1 }]);
  });

  it("notesDoc_save_while_saving_is_ignored", () => {
    const s = dirty({ saving: true });
    const out = notesReducer(s, { type: "save" }, N);
    expect(out.state).toBe(s);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_save_of_a_clean_file_is_ignored", () => {
    const s = doc();
    const out = notesReducer(s, { type: "save" }, N);
    expect(out.state).toBe(s);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_save_under_the_bar_writes_nothing", () => {
    const s = dirty({ conflict: { hash: H2, bytes: buf(7) } });
    const out = notesReducer(s, { type: "save" }, N);
    expect(out.state).toBe(s);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_saved_offers_undo_with_the_line", () => {
    const { state, effects } = run(dirty(), { type: "save" }, { type: "serialized", bytes: bytesOf(9), for: "save" }, { type: "saved", saved: fileSaved(), bytes: bytesOf(9) });
    expect(state).toMatchObject({ dirty: false, saving: false, line: "Saved report.pdf.", lastSave: { snapshot: "snap-1", hash: H2 }, refusedSave: false, conflict: null, fromDraft: false });
    expect(state.base.hash).toBe(H2);
    expect(new Uint8Array(state.base.bytes)).toEqual(bytesOf(9));
    expect(types(effects)).toEqual(["markSaved", "dropDraft", "announce"]);
    expect(announced(effects)).toEqual(["Saved report.pdf."]);
  });

  it("notesDoc_saved_without_a_snapshot_offers_no_undo", () => {
    const { state } = run(dirty({ saving: true }), { type: "saved", saved: fileSaved({ snapshot: null }), bytes: bytesOf(9) });
    expect(state.lastSave).toBeNull();
    expect(state.line).toBe("Saved report.pdf.");
  });

  it("notesDoc_an_edit_during_the_save_stays_unsaved", () => {
    const { state, effects } = run(
      dirty(),
      { type: "save" },
      { type: "serialized", bytes: bytesOf(9), for: "save" },
      { type: "edited" },
      { type: "saved", saved: fileSaved(), bytes: bytesOf(9) },
    );
    expect(state.dirty).toBe(true);
    expect(state.base.hash).toBe(H2);
    expect(effects).toContainEqual({ type: "serialize", then: "draft" });
    expect(effects).not.toContainEqual({ type: "dropDraft" });
  });

  it("notesDoc_conflict_on_save_shows_the_bar_and_keeps_the_notes", () => {
    const { state, effects } = run(dirty(), { type: "save" }, { type: "serialized", bytes: bytesOf(9), for: "save" }, { type: "saveConflict", hash: H2 });
    expect(state).toMatchObject({ dirty: true, saving: false, conflict: { hash: H2, bytes: null } });
    expect(effects.some((e) => e.type === "save")).toBe(false);
    expect(announced(effects)).toEqual(["report.pdf changed on disk. Your notes aren't saved."]);
  });

  it("notesDoc_conflict_on_save_of_a_deleted_file_is_the_deleted_copy", () => {
    const { state, effects } = run(dirty(), { type: "save" }, { type: "serialized", bytes: bytesOf(9), for: "save" }, { type: "saveConflict", hash: null });
    expect(state.conflict).toEqual({ hash: null, bytes: null });
    expect(announced(effects)).toEqual(["report.pdf was deleted. Your notes aren't saved."]);
  });

  it("notesDoc_refused_save_offers_a_copy", () => {
    const { state, effects } = run(dirty(), { type: "save" }, { type: "serialized", bytes: bytesOf(9), for: "save" }, { type: "saveRefused", line: "The file is read-only or in use by another program." });
    expect(state).toMatchObject({ dirty: true, saving: false, refusedSave: true, line: "The file is read-only or in use by another program." });
    expect(announced(effects)).toEqual(["The file is read-only or in use by another program."]);
  });

  it("notesDoc_a_save_can_be_tried_again_after_a_refusal", () => {
    const { state, effects } = run(dirty({ refusedSave: true, line: "x" }), { type: "save" });
    expect(state).toMatchObject({ saving: true, refusedSave: false });
    expect(effects).toEqual([{ type: "serialize", then: "save" }]);
  });
});

describe("notesDoc undo", () => {
  const afterSave = () => doc({ base: { hash: H2, bytes: buf(9) }, lastSave: { snapshot: "snap-1", hash: H2 }, line: "Saved report.pdf." });

  it("notesDoc_unsave_asks_for_the_snapshot_and_waits", () => {
    const { state, effects } = run(afterSave(), { type: "unsave" });
    expect(state.saving).toBe(true);
    expect(effects).toEqual([{ type: "unsave", snapshot: "snap-1" }]);
    expect(run(state, { type: "unsave" }).effects).toEqual([]);
  });

  it("notesDoc_unsave_without_a_save_is_ignored", () => {
    const s = doc();
    expect(notesReducer(s, { type: "unsave" }, N).state).toBe(s);
  });

  it("notesDoc_unsaved_keeps_the_notes_and_makes_it_dirty_again", () => {
    const { state, effects } = run(afterSave(), { type: "unsave" }, { type: "unsaved", hash: H1 });
    expect(state).toMatchObject({ dirty: true, saving: false, lastSave: null, line: "report.pdf is back as it was before the save." });
    expect(state.base.hash).toBe(H1);
    expect(effects).toContainEqual({ type: "serialize", then: "draft" });
    expect(types(effects)).not.toContain("load");
    expect(announced(effects)).toEqual(["report.pdf is back as it was before the save."]);
  });

  it("notesDoc_unsave_refused_says_the_file_changed", () => {
    const { state, effects } = run(afterSave(), { type: "unsave" }, { type: "unsaveRefused" });
    expect(state).toMatchObject({ saving: false, lastSave: null, line: "report.pdf changed after the save, so the save can't be undone." });
    expect(announced(effects)).toEqual(["report.pdf changed after the save, so the save can't be undone."]);
  });

  it("notesDoc_unsave_failed_keeps_the_save_to_undo", () => {
    const { state, effects } = run(afterSave(), { type: "unsave" }, { type: "unsaveFailed", line: "The file is in use." });
    expect(state).toMatchObject({ saving: false, lastSave: { snapshot: expect.any(String) }, line: "The save of report.pdf couldn't be undone. The file is in use." });
    expect(announced(effects)).toEqual(["The save of report.pdf couldn't be undone. The file is in use."]);
  });
});

describe("notesDoc disk", () => {
  it("notesDoc_disk_with_the_base_hash_changes_nothing", () => {
    const s = dirty();
    const out = notesReducer(s, { type: "disk", hash: H1, bytes: buf(1, 2, 3) }, N);
    expect(out.state).toBe(s);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_own_save_is_not_a_disk_change", () => {
    const saved = run(dirty(), { type: "save" }, { type: "serialized", bytes: bytesOf(9), for: "save" }, { type: "saved", saved: fileSaved(), bytes: bytesOf(9) }).state;
    // The notes went on after the save, so the tab is dirty, and the poll then sees the file it wrote.
    const edited = run(saved, { type: "edited" }).state;
    for (const s of [saved, edited]) {
      const out = notesReducer(s, { type: "disk", hash: H2, bytes: buf(9) }, N);
      expect(out.state).toBe(s);
      expect(out.effects).toEqual([]);
    }
  });

  it("notesDoc_the_last_save_hash_is_not_a_change_even_after_a_newer_base", () => {
    const s = dirty({ base: { hash: H3, bytes: buf(5) }, lastSave: { snapshot: "s", hash: H2 } });
    const out = notesReducer(s, { type: "disk", hash: H2, bytes: buf(9) }, N);
    expect(out.state).toBe(s);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_disk_change_while_dirty_is_a_conflict_and_loads_nothing", () => {
    const { state, effects } = run(dirty(), { type: "disk", hash: H3, bytes: buf(7) });
    expect(state.conflict).toEqual({ hash: H3, bytes: buf(7) });
    expect(state.dirty).toBe(true);
    expect(types(effects)).not.toContain("load");
    expect(announced(effects)).toEqual(["report.pdf changed on disk. Your notes aren't saved."]);
  });

  it("notesDoc_the_same_conflict_is_not_announced_twice", () => {
    const once = run(dirty(), { type: "disk", hash: H3, bytes: buf(7) }).state;
    const out = notesReducer(once, { type: "disk", hash: H3, bytes: buf(7) }, N);
    expect(out.state).toBe(once);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_disk_change_while_clean_loads", () => {
    const { state, effects } = run(doc(), { type: "disk", hash: H3, bytes: buf(7) });
    expect(state).toMatchObject({ dirty: false, conflict: null });
    expect(state.base.hash).toBe(H3);
    expect(effects).toEqual([{ type: "load", bytes: buf(7), dirty: false }]);
  });

  it("notesDoc_a_deleted_file_while_dirty_is_a_conflict_with_the_deleted_copy", () => {
    const { state, effects } = run(dirty(), { type: "disk", hash: null, bytes: null });
    expect(state.conflict).toEqual({ hash: null, bytes: null });
    expect(state.dirty).toBe(true);
    expect(announced(effects)).toEqual(["report.pdf was deleted. Your notes aren't saved."]);
  });

  it("notesDoc_a_deleted_file_while_clean_changes_nothing", () => {
    const s = doc();
    const out = notesReducer(s, { type: "disk", hash: null, bytes: null }, N);
    expect(out.state).toBe(s);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_the_disk_coming_back_to_the_base_settles_the_bar", () => {
    const { state } = run(dirty({ conflict: { hash: H3, bytes: buf(7) } }), { type: "disk", hash: H1, bytes: buf(1, 2, 3) });
    expect(state).toMatchObject({ conflict: null, dirty: true });
  });

  it("notesDoc_a_disk_read_while_saving_is_left_for_the_caller", () => {
    const s = dirty({ saving: true });
    const out = notesReducer(s, { type: "disk", hash: H3, bytes: buf(7) }, N);
    expect(out.state).toBe(s);
    expect(out.effects).toEqual([]);
  });

  it("notesDoc_a_conflict_without_bytes_takes_them_from_the_next_read", () => {
    const { state } = run(dirty({ conflict: { hash: H3, bytes: null } }), { type: "disk", hash: H3, bytes: buf(7) });
    expect(state.conflict).toEqual({ hash: H3, bytes: buf(7) });
  });
});

describe("notesDoc bar answers", () => {
  const bar = () => dirty({ conflict: { hash: H3, bytes: buf(7) } });

  it("notesDoc_reload_drops_the_draft_and_loads_the_disk_clean", () => {
    const { state, effects } = run(bar(), { type: "reload" });
    expect(state).toMatchObject({ conflict: null, dirty: false, fromDraft: false, line: null });
    expect(state.base.hash).toBe(H3);
    expect(effects).toEqual([{ type: "load", bytes: buf(7), dirty: false }, { type: "dropDraft" }]);
  });

  it("notesDoc_reload_waits_for_a_read_when_the_bar_has_no_bytes", () => {
    const s = dirty({ conflict: { hash: H3, bytes: null } });
    expect(notesReducer(s, { type: "reload" }, N).state).toBe(s);
  });

  it("notesDoc_reload_without_a_bar_is_ignored", () => {
    const s = dirty();
    expect(notesReducer(s, { type: "reload" }, N).state).toBe(s);
  });

  it("notesDoc_overwrite_saves_with_the_conflict_hash_as_expect", () => {
    const a = run(bar(), { type: "overwrite" });
    expect(a.state.saving).toBe(true);
    expect(a.effects).toEqual([{ type: "serialize", then: "overwrite" }]);
    const b = run(a.state, { type: "serialized", bytes: bytesOf(9), for: "overwrite" });
    expect(b.effects).toEqual([{ type: "save", bytes: bytesOf(9), expect: H3 }]);
  });

  it("notesDoc_overwrite_of_a_deleted_file_expects_no_file", () => {
    const a = run(dirty({ conflict: { hash: null, bytes: null } }), { type: "overwrite" }, { type: "serialized", bytes: bytesOf(9), for: "overwrite" });
    expect(a.effects).toEqual([{ type: "save", bytes: bytesOf(9), expect: null }]);
  });

  it("notesDoc_saved_after_an_overwrite_settles_the_bar", () => {
    const { state } = run(bar(), { type: "overwrite" }, { type: "serialized", bytes: bytesOf(9), for: "overwrite" }, { type: "saved", saved: fileSaved(), bytes: bytesOf(9) });
    expect(state).toMatchObject({ conflict: null, dirty: false, saving: false });
  });

  it("notesDoc_overwrite_while_saving_is_ignored", () => {
    const s = dirty({ saving: true, conflict: { hash: H3, bytes: buf(7) } });
    expect(notesReducer(s, { type: "overwrite" }, N).state).toBe(s);
  });

  it("notesDoc_copy_serializes_then_saves_a_copy_and_says_where", () => {
    const a = run(bar(), { type: "copy" });
    expect(a.effects).toEqual([{ type: "serialize", then: "copy" }]);
    const b = run(a.state, { type: "serialized", bytes: bytesOf(9), for: "copy" });
    expect(b.effects).toEqual([{ type: "saveCopy", bytes: bytesOf(9) }]);
    const c = run(b.state, { type: "copied", name: "report (notes).pdf" });
    expect(c.state.line).toBe("Saved the notes in report (notes).pdf.");
    expect(announced(c.effects)).toEqual(["Saved the notes in report (notes).pdf."]);
    expect(c.state.dirty).toBe(true);
  });
});

describe("notesDoc draft", () => {
  it("notesDoc_draft_serialized_puts_it_while_dirty", () => {
    expect(run(dirty(), { type: "serialized", bytes: bytesOf(9), for: "draft" }).effects).toEqual([{ type: "putDraft" }]);
  });

  it("notesDoc_draft_serialized_after_a_save_drops_it", () => {
    expect(run(doc(), { type: "serialized", bytes: bytesOf(9), for: "draft" }).effects).toEqual([{ type: "dropDraft" }]);
  });

  it("notesDoc_draft_unchanged_file_shows_notes_dirty", () => {
    const { state, effects } = run(doc(), { type: "draft", bytes: buf(8, 8), baseHash: H1 });
    expect(state).toMatchObject({ dirty: true, conflict: null, fromDraft: true });
    expect(state.base.hash).toBe(H1);
    expect(effects).toEqual([{ type: "load", bytes: buf(8, 8), dirty: true }]);
  });

  it("notesDoc_draft_changed_file_is_a_conflict", () => {
    const { state, effects } = run(doc({ base: { hash: H3, bytes: buf(7) } }), { type: "draft", bytes: buf(8, 8), baseHash: H1 });
    expect(state).toMatchObject({ dirty: true, fromDraft: true, conflict: { hash: H3, bytes: buf(7) } });
    expect(state.base.hash).toBe(H1);
    // The draft's notes are shown and unsaved; the disk's file is held back in the bar.
    expect(effects).toContainEqual({ type: "load", bytes: buf(8, 8), dirty: true });
    expect(announced(effects)).toEqual(["report.pdf changed on disk. Your notes aren't saved."]);
  });

  it("notesDoc_draft_deleted_file_is_a_conflict", () => {
    const { state, effects } = run(doc({ base: { hash: null, bytes: new ArrayBuffer(0) } }), { type: "draft", bytes: buf(8, 8), baseHash: H1 });
    expect(state).toMatchObject({ dirty: true, fromDraft: true, conflict: { hash: null, bytes: null } });
    expect(effects).toContainEqual({ type: "load", bytes: buf(8, 8), dirty: true });
    expect(announced(effects)).toEqual(["report.pdf was deleted. Your notes aren't saved."]);
  });

  it("notesDoc_draft_of_a_file_that_was_not_there_and_still_is_not_shows_notes", () => {
    const { state } = run(doc({ base: { hash: null, bytes: new ArrayBuffer(0) } }), { type: "draft", bytes: buf(8), baseHash: null });
    expect(state).toMatchObject({ dirty: true, conflict: null, fromDraft: true });
  });

  it("notesDoc_draft_is_ignored_once_the_tab_has_notes_of_its_own", () => {
    const s = dirty();
    expect(notesReducer(s, { type: "draft", bytes: buf(8), baseHash: H1 }, N).state).toBe(s);
  });

  it("notesDoc_draft_refused_keeps_the_notes_and_says_why", () => {
    const line = "Marshell keeps at most 1 GB of unsaved PDF notes. Save some PDFs first.";
    const { state, effects } = run(dirty(), { type: "draftRefused", line });
    expect(state).toMatchObject({ dirty: true, line });
    expect(types(effects)).not.toContain("load");
    expect(announced(effects)).toEqual([line]);
  });
});

describe("copyName", () => {
  it("copyName_free_takes_notes", () => {
    expect(copyName("report", () => false)).toBe("report (notes).pdf");
  });
  it("copyName_taken_once_counts_on", () => {
    expect(copyName("report", (n) => n === "report (notes).pdf")).toBe("report (notes 2).pdf");
    expect(copyName("report", (n) => n === "report (notes).pdf" || n === "report (notes 2).pdf")).toBe("report (notes 3).pdf");
  });
  it("copyName_all_taken_is_null", () => {
    expect(copyName("report", () => true)).toBeNull();
  });
  it("copyName_stops_at_99", () => {
    expect(copyName("r", (n) => n !== "r (notes 99).pdf")).toBe("r (notes 99).pdf");
  });
});

describe("winAnsiSafe", () => {
  it("winAnsiSafe_ascii_and_whitespace_are_safe", () => {
    expect(winAnsiSafe("Sign here, please.\n\tThanks")).toBe(true);
    expect(winAnsiSafe("")).toBe(true);
  });
  it("winAnsiSafe_accented_latin_and_the_euro_are_safe", () => {
    expect(winAnsiSafe("Café Müller, naïve façade, ñandú, Ærø, ÿ, ½ €5")).toBe(true);
  });
  it("winAnsiSafe_the_0x80_extras_are_safe", () => {
    expect(winAnsiSafe("€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ")).toBe(true);
  });
  it("winAnsiSafe_other_scripts_are_not", () => {
    expect(winAnsiSafe("שלום")).toBe(false);
    expect(winAnsiSafe("Привет")).toBe(false);
    expect(winAnsiSafe("你好")).toBe(false);
    expect(winAnsiSafe("ok → next")).toBe(false);
    expect(winAnsiSafe("a😀")).toBe(false);
  });
  it("winAnsiSafe_a_stray_control_character_is_not", () => {
    expect(winAnsiSafe("a\u0001b")).toBe(false);
    expect(winAnsiSafe("a\u008db")).toBe(false);
  });
});

describe("final review 6q-6: unsaved while a save is in flight", () => {
  it("counts the notes unsaved until the save is answered, so the dot stays and closing the tab asks", () => {
    const sending = run(dirty(), { type: "save" }, { type: "serialized", bytes: bytesOf(1), for: "save" }).state;
    expect(sending.dirty).toBe(false);
    expect(unsavedNotes(sending)).toBe(true);
    const done = run(sending, { type: "saved", saved: fileSaved(), bytes: bytesOf(1) }).state;
    expect(unsavedNotes(done)).toBe(false);
    expect(unsavedNotes(doc())).toBe(false);
    expect(unsavedNotes(dirty())).toBe(true);
  });
});
