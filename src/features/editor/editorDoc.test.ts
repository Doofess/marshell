import { describe, expect, it } from "vitest";
import type { Draft } from "../../generated/Draft";
import { changedLines } from "./changedLines";
import { type DocEvent, type DocState, type Effect, start, step } from "./editorDoc";

const N = "a.txt";
const H1 = "1".repeat(64);
const H2 = "2".repeat(64);
const H3 = "3".repeat(64);

/** A state after opening `text` from the disk at hash H1, with overrides. */
function doc(over: Partial<DocState> = {}): DocState {
  return { ...start, base: { text: "one\ntwo", hash: H1 }, text: "one\ntwo", ...over };
}
const draftOf = (over: Partial<Draft> = {}): Draft => ({
  path: "C:\\a.txt",
  text: "mine",
  base_text: "one\ntwo",
  base_hash: H1,
  encoding: "utf-8",
  eol: "lf",
  edited_ms: 1,
  ...over,
});
const run = (s: DocState, ...events: DocEvent[]) => {
  let state = s;
  let effects: Effect[] = [];
  for (const e of events) {
    const out = step(state, e, N);
    state = out.state;
    effects = out.effects;
  }
  return { state, effects };
};
const types = (effects: Effect[]) => effects.map((e) => e.type);

describe("start", () => {
  it("is clean and empty", () => {
    expect(start).toMatchObject({ text: "", dirty: false, conflict: null, deleted: false, saving: false, lastSave: null, marks: [], line: null, undoOffered: false, sent: null });
  });
});

describe("step", () => {
  it("step_opened_with_no_draft_is_clean", () => {
    const { state, effects } = run(start, { type: "opened", disk: { text: "a\nb", hash: H1 }, draft: null });
    expect(state).toMatchObject({ text: "a\nb", base: { text: "a\nb", hash: H1 }, dirty: false, conflict: null, deleted: false });
    expect(effects).toEqual([]);
  });
  it("opened_with_a_draft_equal_to_the_disk_drops_it", () => {
    const { state, effects } = run(start, { type: "opened", disk: { text: "same", hash: H2 }, draft: draftOf({ text: "same" }) });
    expect(state).toMatchObject({ text: "same", dirty: false, conflict: null, base: { text: "same", hash: H2 } });
    expect(effects).toEqual([{ type: "dropDraft" }]);
  });
  it("step_opened_with_a_draft_over_the_same_disk_is_dirty", () => {
    const { state } = run(start, { type: "opened", disk: { text: "one\ntwo", hash: H1 }, draft: draftOf() });
    expect(state).toMatchObject({ text: "mine", dirty: true, conflict: null, base: { text: "one\ntwo", hash: H1 }, deleted: false });
  });
  it("opened_with_a_draft_over_a_changed_disk_shows_the_bar", () => {
    const disk = { text: "other", hash: H2 };
    const { state } = run(start, { type: "opened", disk, draft: draftOf() });
    expect(state).toMatchObject({ text: "mine", dirty: true, conflict: disk, base: { text: "one\ntwo", hash: H1 } });
  });
  it("opened_with_a_draft_for_a_deleted_file_is_dirty_and_deleted", () => {
    const { state } = run(start, { type: "opened", disk: { text: "", hash: null }, draft: draftOf() });
    expect(state).toMatchObject({ text: "mine", dirty: true, deleted: true, conflict: null });
    expect(state.base.hash).toBeNull();
  });
  it("step_edited_sets_dirty_and_keeps_a_draft", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "one\nthree" });
    expect(state).toMatchObject({ text: "one\nthree", dirty: true });
    expect(effects).toEqual([{ type: "putDraft" }]);
  });
  it("step_edited_back_to_clean_drops_the_draft", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "x" }, { type: "edited", text: "one\ntwo" });
    expect(state.dirty).toBe(false);
    expect(effects).toEqual([{ type: "dropDraft" }]);
  });
  it("step_edited_clears_the_marks_and_the_undo_offer", () => {
    const { state } = run(doc({ marks: [1, 2], undoOffered: true }), { type: "edited", text: "one\ntwo!" });
    expect(state).toMatchObject({ marks: [], undoOffered: false });
  });
  it("step_disk_while_clean_reloads_with_marks_and_an_announcement", () => {
    const { state, effects } = run(doc(), { type: "disk", disk: { text: "one\nTWO\nthree", hash: H2 } });
    expect(state).toMatchObject({ text: "one\nTWO\nthree", dirty: false, base: { text: "one\nTWO\nthree", hash: H2 }, conflict: null });
    expect(effects).toEqual([
      { type: "setText", text: "one\nTWO\nthree", marks: [2, 3] },
      // Only for someone reading in the tab: an agent that writes the file every second must not talk over the terminal.
      { type: "announce", text: "a.txt changed on disk. 2 lines changed.", reading: true },
    ]);
  });
  it("step_disk_while_clean_says_1_line_changed_for_one", () => {
    const { effects } = run(doc(), { type: "disk", disk: { text: "one\nTWO", hash: H2 } });
    expect(effects).toContainEqual({ type: "announce", text: "a.txt changed on disk. 1 line changed.", reading: true });
  });
  it("step_disk_while_dirty_sets_the_conflict_and_keeps_the_text", () => {
    const disk = { text: "other", hash: H2 };
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "disk", disk });
    expect(state).toMatchObject({ text: "mine", dirty: true, conflict: disk, base: { text: "one\ntwo", hash: H1 } });
    expect(types(effects)).not.toContain("setText");
  });
  it("step_a_disk_change_under_edits_never_touches_the_text", () => {
    const edited = run(doc(), { type: "edited", text: "mine" });
    for (const disk of [{ text: "x", hash: H2 }, { text: "y", hash: H3 }, { text: "", hash: null }]) {
      const out = step(edited.state, { type: "disk", disk }, N);
      expect(out.state.text).toBe("mine");
      expect(out.state.dirty).toBe(true);
      expect(types(out.effects)).not.toContain("setText");
    }
  });
  it("step_a_disk_equal_to_base_is_nothing", () => {
    const s = doc();
    const out = step(s, { type: "disk", disk: { text: "one\ntwo", hash: H1 } }, N);
    expect(out.state).toEqual(s);
    expect(out.effects).toEqual([]);
    const dirty = run(doc(), { type: "edited", text: "mine" });
    const same = step(dirty.state, { type: "disk", disk: { text: "one\ntwo", hash: H1 } }, N);
    expect(same.state).toEqual(dirty.state);
    expect(same.effects).toEqual([]);
  });
  it("step_a_disk_equal_to_the_dirty_text_moves_base_and_drops_the_draft", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "disk", disk: { text: "mine", hash: H2 } });
    expect(state).toMatchObject({ dirty: false, conflict: null, text: "mine", base: { text: "mine", hash: H2 } });
    expect(effects).toEqual([{ type: "dropDraft" }]);
  });
  it("step_a_disk_that_goes_back_to_base_clears_a_conflict", () => {
    const { state } = run(doc(), { type: "edited", text: "mine" }, { type: "disk", disk: { text: "x", hash: H2 } }, { type: "disk", disk: { text: "one\ntwo", hash: H1 } });
    expect(state).toMatchObject({ conflict: null, dirty: true, text: "mine" });
  });
  it("step_gone_keeps_the_text_and_is_dirty_when_it_isnt_empty", () => {
    const { state, effects } = run(doc(), { type: "gone" });
    expect(state).toMatchObject({ deleted: true, text: "one\ntwo", dirty: true, conflict: null });
    expect(state.base.hash).toBeNull();
    expect(effects).toEqual([{ type: "putDraft" }]);
  });
  it("step_gone_with_no_text_is_clean", () => {
    const { state } = run(doc({ base: { text: "", hash: H1 }, text: "" }), { type: "gone" });
    expect(state).toMatchObject({ deleted: true, dirty: false });
  });
  it("step_gone_while_edited_stays_dirty_and_keeps_the_edit", () => {
    const { state } = run(doc(), { type: "edited", text: "mine" }, { type: "gone" });
    expect(state).toMatchObject({ deleted: true, dirty: true, text: "mine" });
  });
  it("step_a_file_that_comes_back_under_edits_shows_the_bar", () => {
    const disk = { text: "back", hash: H2 };
    const { state } = run(doc(), { type: "gone" }, { type: "disk", disk });
    expect(state).toMatchObject({ deleted: false, conflict: disk, text: "one\ntwo", dirty: true });
  });
  it("step_save_on_a_dirty_file_expects_the_base_hash", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "save" });
    expect(state).toMatchObject({ saving: true, sent: "mine" });
    expect(effects).toEqual([{ type: "save", expect: H1 }]);
  });
  it("step_save_of_a_deleted_file_expects_none", () => {
    const { effects } = run(doc(), { type: "gone" }, { type: "save" });
    expect(effects).toEqual([{ type: "save", expect: null }]);
  });
  it("step_save_while_clean_with_a_file_is_nothing", () => {
    const s = doc();
    expect(step(s, { type: "save" }, N)).toEqual({ state: s, effects: [] });
  });
  it("step_save_while_a_conflict_is_up_writes_nothing_and_says_why", () => {
    const s = run(doc(), { type: "edited", text: "mine" }, { type: "disk", disk: { text: "x", hash: H2 } }).state;
    const line = "a.txt changed on disk, so it wasn't saved.";
    expect(step(s, { type: "save" }, N)).toEqual({ state: { ...s, line }, effects: [{ type: "announce", text: line }] });
  });
  it("step_save_while_a_conflict_is_up_and_a_save_is_out_is_nothing", () => {
    const s = { ...run(doc(), { type: "edited", text: "mine" }, { type: "disk", disk: { text: "x", hash: H2 } }).state, saving: true };
    expect(step(s, { type: "save" }, N)).toEqual({ state: s, effects: [] });
  });
  it("step_a_second_save_while_saving_is_nothing", () => {
    const s = run(doc(), { type: "edited", text: "mine" }, { type: "save" }).state;
    const out = step(s, { type: "save" }, N);
    expect(out.effects).toEqual([]);
    expect(out.state).toEqual(s);
  });
  it("step_saved_moves_base_to_what_was_sent_and_offers_undo", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" });
    expect(state).toMatchObject({
      base: { text: "mine", hash: H2 },
      dirty: false,
      saving: false,
      sent: null,
      deleted: false,
      lastSave: { snapshot: "snap", hash: H2 },
      undoOffered: true,
      line: "Saved a.txt.",
    });
    expect(effects).toEqual([{ type: "dropDraft" }, { type: "refetch" }, { type: "announce", text: "Saved a.txt." }]);
  });
  it("step_saved_with_no_snapshot_leaves_lastSave_null_and_offers_no_undo", () => {
    const { state } = run(doc(), { type: "gone" }, { type: "save" }, { type: "saved", hash: H2, snapshot: null });
    expect(state).toMatchObject({ lastSave: null, undoOffered: false, deleted: false, base: { text: "one\ntwo", hash: H2 }, dirty: false, line: "Saved a.txt." });
  });
  it("step_text_typed_while_saving_stays_dirty_after_saved", () => {
    const { state, effects } = run(
      doc(),
      { type: "edited", text: "mine" },
      { type: "save" },
      { type: "edited", text: "mine, more" },
      { type: "saved", hash: H2, snapshot: "snap" },
    );
    expect(state).toMatchObject({ text: "mine, more", base: { text: "mine", hash: H2 }, dirty: true, saving: false });
    expect(types(effects)).toContain("putDraft");
    expect(types(effects)).not.toContain("dropDraft");
  });
  it("step_saved_clears_a_conflict_that_is_our_own_write", () => {
    const { state } = run(
      doc(),
      { type: "edited", text: "mine" },
      { type: "save" },
      { type: "edited", text: "mine!" },
      { type: "disk", disk: { text: "mine", hash: H2 } },
      { type: "saved", hash: H2, snapshot: "snap" },
    );
    expect(state).toMatchObject({ conflict: null, dirty: true, base: { text: "mine", hash: H2 } });
  });
  it("step_save_conflict_asks_for_the_disk_and_says_so", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saveConflict", hash: H2 });
    expect(state).toMatchObject({ saving: false, sent: null, dirty: true, text: "mine", line: "a.txt changed on disk, so it wasn't saved." });
    expect(effects).toEqual([{ type: "refetch" }, { type: "announce", text: "a.txt changed on disk, so it wasn't saved." }]);
  });
  it("step_the_disk_after_a_save_conflict_sets_the_bar", () => {
    const disk = { text: "other", hash: H2 };
    const { state } = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saveConflict", hash: H2 }, { type: "disk", disk });
    expect(state).toMatchObject({ conflict: disk, text: "mine" });
  });
  it("step_save_failed_ends_saving_and_keeps_the_text_dirty", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saveFailed", line: "The file is read-only or in use by another program." });
    const line = "a.txt couldn't be saved. The file is read-only or in use by another program.";
    expect(state).toMatchObject({ saving: false, sent: null, dirty: true, text: "mine", line });
    expect(effects).toEqual([{ type: "announce", text: line }]);
  });
  it("step_keep_mine_moves_base_to_the_disk_and_keeps_the_draft", () => {
    const disk = { text: "other", hash: H2 };
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "disk", disk }, { type: "keepMine" });
    expect(state).toMatchObject({ conflict: null, dirty: true, text: "mine", base: disk });
    expect(effects).toEqual([{ type: "putDraft" }]);
  });
  it("step_keep_mine_then_save_expects_the_disks_new_hash", () => {
    const { effects } = run(
      doc(),
      { type: "edited", text: "mine" },
      { type: "disk", disk: { text: "other", hash: H2 } },
      { type: "keepMine" },
      { type: "save" },
    );
    expect(effects).toEqual([{ type: "save", expect: H2 }]);
  });
  it("step_take_disk_replaces_the_text_and_drops_the_draft", () => {
    const disk = { text: "other", hash: H2 };
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "disk", disk }, { type: "takeDisk" });
    expect(state).toMatchObject({ text: "other", conflict: null, dirty: false, base: disk, marks: [] });
    expect(effects).toEqual([{ type: "setText", text: "other", marks: [] }, { type: "dropDraft" }]);
  });
  it("step_keep_mine_and_take_disk_do_nothing_without_a_conflict", () => {
    const s = doc();
    expect(step(s, { type: "keepMine" }, N)).toEqual({ state: s, effects: [] });
    expect(step(s, { type: "takeDisk" }, N)).toEqual({ state: s, effects: [] });
  });
  it("step_keep_mine_and_take_disk_wait_for_a_save_in_flight", () => {
    // An undo can start with the bar up; a read of the disk during a save is held, so it raises no bar then.
    const s = doc({ text: "mine", dirty: true, saving: true, lastSave: { snapshot: "snap", hash: H1 }, conflict: { text: "other", hash: H2 } });
    expect(step(s, { type: "keepMine" }, N)).toEqual({ state: s, effects: [] });
    expect(step(s, { type: "takeDisk" }, N)).toEqual({ state: s, effects: [] });
  });
  it("step_unsave_asks_for_the_snapshot_only_with_a_last_save", () => {
    const saved = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;
    expect(step(saved, { type: "unsave" }, N).effects).toEqual([{ type: "unsave", snapshot: "snap" }]);
    const none = doc();
    expect(step(none, { type: "unsave" }, N)).toEqual({ state: none, effects: [] });
  });
  it("step_unsave_while_saving_is_nothing", () => {
    const s = run(doc({ lastSave: { snapshot: "snap", hash: H1 } }), { type: "edited", text: "mine" }, { type: "save" }).state;
    expect(step(s, { type: "unsave" }, N).effects).toEqual([]);
  });
  it("step_unsaved_refetches_and_says_the_file_is_back", () => {
    const saved = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;
    const { state, effects } = run(saved, { type: "unsaved", hash: H1 });
    expect(state).toMatchObject({ lastSave: null, undoOffered: false, line: "a.txt is back as it was before the save." });
    expect(effects).toEqual([{ type: "refetch" }, { type: "announce", text: "a.txt is back as it was before the save." }]);
  });
  it("step_the_disk_after_an_unsave_leaves_the_text_which_is_unsaved_again", () => {
    // PLAN §9, 6q-4 decision 8: undo puts the file back, and the editor keeps the person's text.
    const saved = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;
    const { state, effects } = run(saved, { type: "unsaved", hash: H1 }, { type: "disk", disk: { text: "one\ntwo", hash: H1 } });
    expect(state).toMatchObject({ text: "mine", dirty: true, conflict: null, base: { text: "one\ntwo", hash: H1 } });
    expect(effects).toEqual([{ type: "putDraft" }]);
  });
  it("step_a_disk_read_during_an_undo_waits_for_its_answer_and_the_text_stays", () => {
    // The once-a-second check can read the bytes the undo put back before the undo is answered.
    const saved = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;
    const during = run(saved, { type: "unsave" }, { type: "disk", disk: { text: "one\ntwo", hash: H1 } });
    expect(during.state).toMatchObject({ text: "mine", saving: true });
    expect(during.effects).toEqual([]);
    const { state, effects } = run(during.state, { type: "unsaved", hash: H1 });
    expect(state).toMatchObject({ text: "mine", dirty: true, conflict: null, base: { text: "one\ntwo", hash: H1 }, held: null });
    expect(types(effects)).toEqual(["refetch", "announce", "putDraft"]);
  });
  it("step_a_read_of_the_tabs_own_write_during_a_save_raises_no_bar", () => {
    const typing = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "edited", text: "mine and more" }).state;
    const during = run(typing, { type: "disk", disk: { text: "mine", hash: H2 } });
    expect(during.state.conflict).toBeNull();
    const { state } = run(during.state, { type: "saved", hash: H2, snapshot: "snap" });
    expect(state).toMatchObject({ text: "mine and more", dirty: true, conflict: null, base: { text: "mine", hash: H2 }, held: null });
  });
  it("step_somebody_elses_change_read_during_a_save_sets_the_bar_when_the_save_is_answered", () => {
    const saving = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "disk", disk: { text: "theirs", hash: H3 } }).state;
    expect(saving.conflict).toBeNull();
    const { state } = run(saving, { type: "saveConflict", hash: H3 });
    expect(state).toMatchObject({ text: "mine", dirty: true, saving: false, conflict: { text: "theirs", hash: H3 }, held: null });
  });
  it("step_a_file_that_went_during_a_save_is_gone_when_the_save_is_answered", () => {
    const saving = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "gone" }).state;
    expect(saving.deleted).toBe(false);
    const { state } = run(saving, { type: "saveFailed", line: "x" });
    expect(state).toMatchObject({ text: "mine", deleted: true, saving: false, held: null });
  });
  it("step_only_the_newest_read_during_a_save_is_kept", () => {
    const saving = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "disk", disk: { text: "theirs", hash: H3 } }, { type: "disk", disk: { text: "mine", hash: H2 } }).state;
    const { state } = run(saving, { type: "saved", hash: H2, snapshot: "snap" });
    expect(state).toMatchObject({ dirty: false, conflict: null });
  });
  it("step_undo_after_a_later_edit_keeps_the_edit", () => {
    const saved = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }, { type: "edited", text: "mine and more" }).state;
    const { state, effects } = run(saved, { type: "unsaved", hash: H1 }, { type: "disk", disk: { text: "one\ntwo", hash: H1 } });
    expect(state).toMatchObject({ text: "mine and more", dirty: true, conflict: null, base: { text: "one\ntwo", hash: H1 } });
    expect(effects).toEqual([{ type: "putDraft" }]);
  });
  it("step_unsave_refused_says_so_and_clears_lastSave", () => {
    const saved = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;
    const line = "a.txt changed after the save, so the save can't be undone.";
    const { state, effects } = run(saved, { type: "unsaveRefused" });
    expect(state).toMatchObject({ lastSave: null, undoOffered: false, line });
    expect(effects).toEqual([{ type: "announce", text: line }]);
  });
  it("step_left_clears_the_marks_and_the_undo_offer", () => {
    const { state } = run(doc({ marks: [3], undoOffered: true, lastSave: { snapshot: "s", hash: H1 } }), { type: "left" });
    expect(state).toMatchObject({ marks: [], undoOffered: false });
  });
  it("step_never_changes_its_input", () => {
    const s = doc({ marks: [1] });
    const frozen = JSON.stringify(s);
    Object.freeze(s);
    for (const e of [{ type: "edited", text: "x" }, { type: "disk", disk: { text: "y", hash: H2 } }, { type: "gone" }, { type: "save" }, { type: "left" }] as DocEvent[]) {
      step(s, e, N);
    }
    expect(JSON.stringify(s)).toBe(frozen);
  });
});

describe("a read of the file from before a write of the tab's own", () => {
  const savedState = () => run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;

  it("a_save_reads_the_file_again", () => {
    const { effects } = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" });
    expect(types(effects)).toEqual(["dropDraft", "refetch", "announce"]);
  });
  it("a_stale_read_after_a_save_is_dropped_and_the_file_is_read_again", () => {
    // The check that was reading the file when the save landed answers with the old bytes.
    const { state, effects } = run(savedState(), { type: "disk", disk: { text: "one\ntwo", hash: H1 } });
    expect(state).toMatchObject({ text: "mine", base: { text: "mine", hash: H2 }, dirty: false, conflict: null, marks: [], line: "Saved a.txt." });
    expect(effects).toEqual([{ type: "refetch" }]);
  });
  it("text_typed_after_a_stale_read_raises_no_bar", () => {
    const { state } = run(savedState(), { type: "disk", disk: { text: "one\ntwo", hash: H1 } }, { type: "edited", text: "mine!" }, { type: "disk", disk: { text: "mine", hash: H2 } });
    expect(state).toMatchObject({ text: "mine!", dirty: true, conflict: null, base: { text: "mine", hash: H2 } });
  });
  it("a_file_really_put_back_to_its_old_bytes_is_taken_on_the_second_read", () => {
    const old: DocEvent = { type: "disk", disk: { text: "one\ntwo", hash: H1 } };
    const { state, effects } = run(savedState(), old, old);
    expect(state).toMatchObject({ text: "one\ntwo", base: { text: "one\ntwo", hash: H1 }, dirty: false });
    expect(types(effects)).toEqual(["setText", "announce"]);
  });
  it("another_programs_change_after_a_save_is_not_dropped", () => {
    const { state, effects } = run(savedState(), { type: "disk", disk: { text: "theirs", hash: H3 } });
    expect(state).toMatchObject({ text: "theirs", base: { text: "theirs", hash: H3 } });
    expect(types(effects)).toEqual(["setText", "announce"]);
  });
  it("the_disk_as_saved_ends_the_watch_for_a_stale_read", () => {
    const now: DocEvent = { type: "disk", disk: { text: "mine", hash: H2 } };
    const { state, effects } = run(savedState(), now, { type: "disk", disk: { text: "one\ntwo", hash: H1 } });
    expect(state).toMatchObject({ text: "one\ntwo", base: { text: "one\ntwo", hash: H1 } });
    expect(types(effects)).toEqual(["setText", "announce"]);
  });
  it("a_stale_missing_after_a_save_that_made_the_file_again_is_dropped", () => {
    const made = run(doc(), { type: "gone" }, { type: "save" }, { type: "saved", hash: H2, snapshot: null }).state;
    const { state, effects } = run(made, { type: "gone" });
    expect(state).toMatchObject({ deleted: false, dirty: false, base: { text: "one\ntwo", hash: H2 } });
    expect(effects).toEqual([{ type: "refetch" }]);
  });
  it("a_stale_read_after_an_undone_save_is_dropped", () => {
    const undone = run(savedState(), { type: "unsave" }, { type: "unsaved", hash: H1 }).state;
    const { state, effects } = run(undone, { type: "disk", disk: { text: "mine", hash: H2 } });
    expect(state).toMatchObject({ text: "mine", conflict: null, base: { hash: H1 } });
    expect(effects).toEqual([{ type: "refetch" }]);
  });
});

describe("an undo in flight", () => {
  const savedState = () => run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;

  it("a_second_undo_while_one_is_in_flight_is_nothing", () => {
    const first = step(savedState(), { type: "unsave" }, N);
    expect(types(first.effects)).toEqual(["unsave"]);
    expect(step(first.state, { type: "unsave" }, N)).toEqual({ state: first.state, effects: [] });
  });
  it("a_save_waits_for_an_undo_in_flight", () => {
    const s = run(savedState(), { type: "edited", text: "more" }, { type: "unsave" }).state;
    expect(step(s, { type: "save" }, N)).toEqual({ state: s, effects: [] });
  });
  it("an_undo_that_is_answered_lets_a_save_go_again", () => {
    for (const answer of [{ type: "unsaved", hash: H1 }, { type: "unsaveRefused" }, { type: "unsaveFailed", line: "x" }] as DocEvent[]) {
      const s = run(savedState(), { type: "edited", text: "more" }, { type: "unsave" }, answer).state;
      expect(s.saving, answer.type).toBe(false);
      expect(types(step(s, { type: "save" }, N).effects), answer.type).toEqual(["save"]);
    }
  });
  it("an_undo_the_core_refused_says_its_line_and_keeps_the_save", () => {
    const { state, effects } = run(savedState(), { type: "unsave" }, { type: "unsaveFailed", line: "The file is read-only or in use by another program." });
    const text = "The save of a.txt couldn't be undone. The file is read-only or in use by another program.";
    expect(state).toMatchObject({ line: text, lastSave: { snapshot: "snap", hash: H2 }, text: "mine", dirty: false });
    expect(effects).toEqual([{ type: "announce", text }]);
  });
});

describe("the status line", () => {
  it("a_save_over_2_mb_says_the_plans_line", () => {
    const { state, effects } = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saveTooLarge" });
    expect(state).toMatchObject({ saving: false, sent: null, dirty: true, text: "mine", line: "a.txt is over 2 MB, so it can't be saved." });
    expect(effects).toEqual([{ type: "announce", text: "a.txt is over 2 MB, so it can't be saved." }]);
  });
  it("an_edit_clears_a_line_that_is_no_longer_true", () => {
    const saved = run(doc(), { type: "edited", text: "mine" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }).state;
    expect(saved.line).toBe("Saved a.txt.");
    expect(step(saved, { type: "edited", text: "mine!" }, N).state.line).toBeNull();
  });
});

describe("the merge", () => {
  const B = { text: "one\ntwo", hash: H1 };
  const M = "mine";
  const D = { text: "other", hash: H2 };
  const D2 = { text: "other again", hash: H3 };
  const MERGED = "a\n<<<<<<< mine\nmine\n=======\nother\n>>>>>>> disk";
  const lines = (t: string) => t.split("\n");
  const merged = (over: Partial<Extract<DocEvent, { type: "merged" }>> = {}): DocEvent => ({ type: "merged", id: 1, text: MERGED, conflicts: 1, mine: M, disk: H2, ...over });
  /** Base B, text M, the bar up with disk D. */
  const barUp = () => run(start, { type: "opened", disk: D, draft: draftOf() }).state;
  const merging = () => run(barUp(), { type: "merge" }).state;
  const done = (over: Partial<Extract<DocEvent, { type: "merged" }>> = {}) => run(merging(), merged(over));

  it("start_is_not_merging", () => {
    expect(start).toMatchObject({ merging: false, merge: null });
  });
  it("merge_asks_for_the_three_texts", () => {
    const { state, effects } = run(barUp(), { type: "merge" });
    expect(effects).toEqual([{ type: "merge", id: 1, base: B.text, mine: M, disk: D }]);
    expect(state).toMatchObject({ merging: true, line: "Merging a.txt…" });
    expect(types(effects)).not.toContain("announce");
  });
  it("merge_does_nothing_without_the_bar_while_saving_or_twice", () => {
    const quiet = (s: DocState) => expect(step(s, { type: "merge" }, N)).toEqual({ state: s, effects: [] });
    quiet(run(doc(), { type: "edited", text: M }).state);
    quiet({ ...barUp(), saving: true });
    quiet(merging());
  });
  it("merged_puts_the_text_in_and_takes_the_disk_as_the_base", () => {
    const { state, effects } = done({ conflicts: 2 });
    const line = "Merged a.txt. 2 conflicts to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge.";
    const marks = changedLines(lines(M), lines(MERGED));
    expect(state).toMatchObject({ text: MERGED, base: D, conflict: null, deleted: false, dirty: true, merging: false, marks, line });
    expect(state.merge).toEqual({ mine: M, merged: MERGED, before: B, after: D });
    expect(effects).toEqual([{ type: "setText", text: MERGED, marks }, { type: "showConflict" }, { type: "putDraft" }, { type: "announce", text: line }]);
  });
  it("merged_with_one_conflict_and_with_none_say_so", () => {
    const one = done({ conflicts: 1 });
    expect(one.state.line).toBe("Merged a.txt. 1 conflict to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge.");
    expect(one.effects[3]).toEqual({ type: "announce", text: one.state.line });
    const none = done({ text: "one\nmerged", conflicts: 0 });
    expect(none.state.line).toBe("Merged a.txt. No conflicts. Ctrl+Z undoes the merge.");
    expect(types(none.effects)).toEqual(["setText", "putDraft", "announce"]);
    expect(none.effects[2]).toEqual({ type: "announce", text: none.state.line });
  });
  it("merged_to_the_disks_own_text_is_clean", () => {
    const { state, effects } = done({ text: D.text, conflicts: 0 });
    expect(state).toMatchObject({ text: D.text, base: D, conflict: null, dirty: false, merging: false });
    expect(types(effects)).toEqual(["setText", "dropDraft", "announce"]);
  });
  it("merged_to_the_persons_own_text_remembers_nothing", () => {
    const { state, effects } = done({ text: M, conflicts: 0 });
    expect(state).toMatchObject({ text: M, base: D, conflict: null, merge: null, line: "Merged a.txt. No conflicts." });
    expect(effects.at(-1)).toEqual({ type: "announce", text: "Merged a.txt. No conflicts." });
  });
  it("merged_for_an_older_text_runs_again", () => {
    const { state, effects } = run(merging(), { type: "edited", text: "mine2" }, merged());
    expect(state).toMatchObject({ text: "mine2", base: B, conflict: D, merging: true, line: "Merging a.txt…" });
    expect(effects).toEqual([{ type: "merge", id: 2, base: B.text, mine: "mine2", disk: D }]);
  });
  it("merged_for_an_older_disk_runs_again", () => {
    const { state, effects } = run(merging(), { type: "disk", disk: D2 }, merged());
    expect(state).toMatchObject({ text: M, base: B, conflict: D2, merging: true });
    expect(effects).toEqual([{ type: "merge", id: 2, base: B.text, mine: M, disk: D2 }]);
  });
  it("merged_after_keep_mine_or_take_disk_is_dropped", () => {
    for (const answer of [{ type: "keepMine" }, { type: "takeDisk" }] as DocEvent[]) {
      const before = run(merging(), answer).state;
      const { state, effects } = run(before, merged());
      expect(effects, answer.type).toEqual([]);
      expect(state, answer.type).toEqual({ ...before, merging: false });
    }
  });
  it("answering_the_bar_ends_the_merge_so_a_later_bar_is_not_merged_unasked", () => {
    for (const answer of [{ type: "keepMine" }, { type: "takeDisk" }] as DocEvent[]) {
      const answered = run(merging(), answer).state;
      expect(answered, answer.type).toMatchObject({ merging: false, line: null });
      // The file changes again before the old answer comes: the new bar is the person's to answer.
      const again = run(answered, { type: "edited", text: M }, { type: "disk", disk: D2 }).state;
      expect(again.conflict, answer.type).toEqual(D2);
      for (const late of [merged(), merged({ disk: H3 }), { type: "mergeFailed", id: 1, line: "The merge took too long." }] as DocEvent[]) {
        expect(step(again, late, N), `${answer.type} ${late.type}`).toEqual({ state: again, effects: [] });
      }
    }
  });
  it("a_disk_that_comes_to_hold_the_text_ends_the_merge", () => {
    const same = run(merging(), { type: "disk", disk: { text: M, hash: H3 } }).state;
    expect(same).toMatchObject({ merging: false, conflict: null, line: null });
    expect(step(same, merged(), N)).toEqual({ state: same, effects: [] });
  });
  it("a_file_deleted_during_a_merge_drops_the_answer", () => {
    const gone = run(merging(), { type: "gone" }).state;
    expect(gone).toMatchObject({ merging: false, merge: null, conflict: null, deleted: true, line: null });
    const { state, effects } = run(gone, merged());
    expect(effects).toEqual([]);
    expect(state).toEqual(gone);
    expect(state.text).toBe(M);
  });
  it("an_answer_of_an_earlier_merge_is_dropped_while_a_later_one_runs", () => {
    // Merge, Keep mine, the file changes again, Merge again: the first request is still out when the second is made.
    const second = run(merging(), { type: "keepMine" }, { type: "disk", disk: D2 }, { type: "merge" });
    expect(second.effects.at(-1)).toEqual({ type: "merge", id: 2, base: D.text, mine: M, disk: D2 });
    for (const late of [merged(), merged({ disk: H3 }), { type: "mergeFailed", id: 1, line: "Another merge is still running." }] as DocEvent[]) {
      expect(step(second.state, late, N), late.type).toEqual({ state: second.state, effects: [] });
    }
    const { state } = run(second.state, merged({ id: 2, disk: H3 }));
    expect(state).toMatchObject({ text: MERGED, base: D2, conflict: null, merging: false });
  });
  it("a_bar_that_goes_any_other_way_ends_the_merge_too", () => {
    // The file goes back to what the tab loaded.
    const back = run(merging(), { type: "disk", disk: B }).state;
    expect(back).toMatchObject({ merging: false, conflict: null, line: null });
    // It changes again before the old answer comes: nobody asked for a merge of this bar.
    const again = run(back, { type: "disk", disk: D2 }).state;
    expect(again.conflict).toEqual(D2);
    expect(step(again, merged(), N)).toEqual({ state: again, effects: [] });
    // The person types the disk's own text.
    expect(run(merging(), { type: "edited", text: D.text }).state).toMatchObject({ merging: false, conflict: null, line: null });
    // A merge is undone, asked for again, and redone before the answer comes.
    const redone = run(done().state, { type: "edited", text: M }, { type: "merge" }, { type: "edited", text: MERGED }).state;
    expect(redone).toMatchObject({ merging: false, conflict: null, base: D, line: null });
  });
  it("typing_during_a_merge_keeps_the_line_that_says_it_runs", () => {
    expect(run(merging(), { type: "edited", text: "mine2" }).state).toMatchObject({ merging: true, line: "Merging a.txt…" });
  });
  it("merge_failed_keeps_the_bar_and_says_why", () => {
    const { state, effects } = run(merging(), { type: "mergeFailed", id: 1, line: "The merge took too long." });
    const line = "a.txt couldn't be merged. The merge took too long.";
    expect(state).toMatchObject({ merging: false, conflict: D, text: M, line });
    expect(effects).toEqual([{ type: "announce", text: line }]);
  });

  it("undo_of_a_merge_brings_the_bar_back", () => {
    const { state, effects } = run(done().state, { type: "edited", text: M });
    expect(state).toMatchObject({ text: M, base: B, conflict: D, dirty: true });
    expect(state.merge).not.toBeNull();
    expect(effects).toEqual([{ type: "putDraft" }]);
    const refused = step(state, { type: "save" }, N);
    expect(refused.effects).toEqual([{ type: "announce", text: "a.txt changed on disk, so it wasn't saved." }]);
  });
  it("redo_of_a_merge_takes_the_bar_away", () => {
    const back = run(done().state, { type: "edited", text: M }).state;
    const { state, effects } = run(back, { type: "edited", text: MERGED });
    expect(state).toMatchObject({ text: MERGED, base: D, conflict: null, dirty: true });
    expect(effects).toEqual([{ type: "putDraft" }]);
    expect(step(state, { type: "save" }, N).effects).toEqual([{ type: "save", expect: H2 }]);
  });
  it("undo_past_a_merge_after_more_edits", () => {
    const more = run(done().state, { type: "edited", text: "something else" }).state;
    expect(more).toMatchObject({ base: D, conflict: null });
    const { state } = run(more, { type: "edited", text: M });
    expect(state).toMatchObject({ text: M, base: B, conflict: D, dirty: true });
  });
  it("undo_of_a_merge_after_the_file_changed_again", () => {
    const newer = run(done().state, { type: "disk", disk: D2 }).state;
    expect(newer.conflict).toEqual(D2);
    const back = run(newer, { type: "edited", text: M }).state;
    expect(back).toMatchObject({ base: B, conflict: D2 });
    const again = run(back, { type: "edited", text: MERGED }).state;
    expect(again).toMatchObject({ base: D, conflict: D2, dirty: true });
  });
  it("undo_after_a_save_brings_nothing_back", () => {
    const { state } = run(done({ text: "one\nmerged", conflicts: 0 }).state, { type: "save" }, { type: "saved", hash: "4".repeat(64), snapshot: null }, { type: "edited", text: M });
    expect(state).toMatchObject({ conflict: null, base: { text: "one\nmerged", hash: "4".repeat(64) }, merge: null });
  });
  it("keep_mine_and_take_disk_forget_the_merge", () => {
    for (const answer of [{ type: "keepMine" }, { type: "takeDisk" }] as DocEvent[]) {
      const back = run(done().state, { type: "edited", text: M }).state;
      const { state } = run(back, answer, { type: "edited", text: MERGED });
      expect(state, answer.type).toMatchObject({ conflict: null, base: D, merge: null });
    }
  });
  it("saved_counts_the_conflicts_left", () => {
    const one = "<<<<<<< mine\na\n=======\nb\n>>>>>>> disk";
    const saveOf = (text: string) => run(doc(), { type: "edited", text }, { type: "save" }, { type: "saved", hash: H2, snapshot: null }).state.line;
    expect(saveOf(one)).toBe("Saved a.txt. 1 conflict left.");
    expect(saveOf(`${one}\nx\n${one}`)).toBe("Saved a.txt. 2 conflicts left.");
    expect(saveOf("plain")).toBe("Saved a.txt.");
  });
});
