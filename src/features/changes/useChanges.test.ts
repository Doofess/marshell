import { describe, expect, it } from "vitest";
import type { ChangesReply } from "../../generated/ChangesReply";
import { CHANGES_POLL_MS, changesSchedule, changesState, keepReply } from "./useChanges";

const base = { inFlight: false, shown: true, focused: true, urgent: false, lastAsk: 0 as number | null, now: 0 };

describe("changesSchedule", () => {
  it("asks nothing while hidden or in flight", () => {
    expect(changesSchedule({ ...base, shown: false, urgent: true, lastAsk: null })).toBe("none");
    expect(changesSchedule({ ...base, inFlight: true, urgent: true, lastAsk: null })).toBe("none");
    expect(changesSchedule({ ...base, shown: false, now: 10 * CHANGES_POLL_MS })).toBe("none");
  });
  it("asks at once when urgent and every 2 s while focused", () => {
    expect(changesSchedule({ ...base, lastAsk: null })).toBe("ask");
    expect(changesSchedule({ ...base, urgent: true, now: 1 })).toBe("ask");
    expect(changesSchedule({ ...base, now: CHANGES_POLL_MS - 1 })).toBe("none");
    expect(changesSchedule({ ...base, now: CHANGES_POLL_MS })).toBe("ask");
  });
  it("waits while the window is unfocused", () => {
    expect(changesSchedule({ ...base, focused: false, now: 10 * CHANGES_POLL_MS })).toBe("none");
    expect(changesSchedule({ ...base, focused: false, urgent: true, now: 1 })).toBe("ask");
  });
});

describe("keepReply", () => {
  const reply = (stamp: string, kind: ChangesReply["kind"] = "listed"): ChangesReply => ({ kind, stamp, folder: "C:/p", moved: false, branch: "main", detached: null, base: null, unrelated: false, ahead: 0, uncommitted: false, files: [], diff: "", truncated: false, more_new: 0, new_cut: false, reason: null, head: null, pr_base_unfetched: null });
  it("drops a reply for a session no longer shown", () => {
    const cur = reply("a");
    expect(keepReply({ asked: "s1", shown: "s2" }, cur, reply("b"))).toBe(cur);
  });
  it("takes a new reply and keeps the old one on same", () => {
    const cur = reply("a");
    const next = reply("b");
    expect(keepReply({ asked: "s1", shown: "s1" }, cur, next)).toBe(next);
    expect(keepReply({ asked: "s1", shown: "s1" }, cur, reply("a", "same"))).toBe(cur);
    expect(keepReply({ asked: "s1", shown: "s1" }, null, reply("a", "same"))).toBeNull();
  });
});

describe("changesState", () => {
  const reply = { kind: "listed", stamp: "a" } as ChangesReply;
  it("keeps the state object when a reply changes nothing, so a poll doesn't draw the panel again", () => {
    const prev = { session: "s1", reply, noFolder: false };
    expect(changesState(prev, { session: "s1", reply, noFolder: false })).toBe(prev);
  });
  it("takes a new reply, another session or a change of folder", () => {
    const prev = { session: "s1", reply, noFolder: false };
    const other = { kind: "listed", stamp: "b" } as ChangesReply;
    expect(changesState(prev, { session: "s1", reply: other, noFolder: false }).reply).toBe(other);
    expect(changesState(prev, { session: "s2", reply, noFolder: false }).session).toBe("s2");
    expect(changesState(prev, { session: "s1", reply: null, noFolder: true }).noFolder).toBe(true);
  });
});
