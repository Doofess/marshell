import { describe, expect, it } from "vitest";
import { COMMANDS, MOVED_COMMANDS } from "../../lib/keymap";
import { MODE_IDS, emptyBox, paletteEscape, recordsRecent, withModeStep } from "./paletteList";

const ids = (cs: { id: string }[]) => cs.map((c) => c.id);

describe("paletteList", () => {
  it("recent_drops_unknown_and_moved_ids", () => {
    expect(ids(emptyBox(COMMANDS, ["gone", "appearance", "find"], MOVED_COMMANDS).recent)).toEqual(["find"]);
  });
  it("recent_lists_only_commands_that_are_offered", () => {
    const offered = COMMANDS.filter((c) => c.id !== "find");
    expect(emptyBox(offered, ["find", "close"], MOVED_COMMANDS).recent.map((c) => c.id)).toEqual(["close"]);
  });
  it("the_empty_box_leaves_out_moved_items_mode_rows_and_what_recent_shows", () => {
    const { recent, rest } = emptyBox(COMMANDS, ["find"], MOVED_COMMANDS);
    const r = ids(rest);
    for (const id of [...MOVED_COMMANDS, ...MODE_IDS, ...ids(recent)]) expect(r).not.toContain(id);
    expect(r).toContain("change-mode");
  });
  it("recent_shows_at_most_five", () => {
    const six = COMMANDS.filter((c) => !MOVED_COMMANDS.includes(c.id)).slice(0, 8).map((c) => c.id);
    expect(emptyBox(COMMANDS, six, MOVED_COMMANDS).recent).toHaveLength(5);
  });
  it("rest_is_in_group_order", () => {
    const order = ["Session", "Terminal", "Window", "App"];
    const groups = emptyBox([...COMMANDS].reverse(), [], MOVED_COMMANDS).rest.map((c) => order.indexOf(c.group));
    expect(groups).toEqual([...groups].sort((a, b) => a - b));
  });
  it("the_mode_step_goes_when_no_mode_is_offered", () => {
    expect(ids(withModeStep(COMMANDS))).toContain("change-mode");
    expect(ids(withModeStep(COMMANDS.filter((c) => !MODE_IDS.includes(c.id))))).not.toContain("change-mode");
  });
  it("only_palette_runs_of_listed_commands_are_recorded", () => {
    expect(recordsRecent("find")).toBe(true);
    for (const id of ["session:3", "file:a.ts", "change-mode", "appearance", "palette"]) expect(recordsRecent(id), id).toBe(false);
  });
  it("escape_goes_back_from_the_mode_list_then_closes", () => {
    expect(paletteEscape("modes")).toBe("back");
    expect(paletteEscape("commands")).toBe("close");
  });
});
