import { describe, expect, it } from "vitest";
import { askedAt, githubSchedule, recountKey } from "./useGithub";

const base = { inFlight: false, shown: true, focused: true, on: true, lastAsk: 0, nextMs: 30_000, now: 0, headMoved: false, asked: true };

describe("githubSchedule", () => {
  it("asks every 30 s, and every 10 s while checks run", () => {
    expect(githubSchedule({ ...base, now: 29_999 })).toBe("none");
    expect(githubSchedule({ ...base, now: 30_000 })).toBe("ask");
    expect(githubSchedule({ ...base, nextMs: 10_000, now: 9_999 })).toBe("none");
    expect(githubSchedule({ ...base, nextMs: 10_000, now: 10_000 })).toBe("ask");
  });
  it("asks at once when nothing was asked yet", () => {
    expect(githubSchedule({ ...base, asked: false })).toBe("ask");
  });
  it("asks nothing while hidden, unfocused, off or in flight", () => {
    const due = { ...base, asked: false, now: 99_999, headMoved: true };
    expect(githubSchedule({ ...due, shown: false })).toBe("none");
    expect(githubSchedule({ ...due, focused: false })).toBe("none");
    expect(githubSchedule({ ...due, on: false })).toBe("none");
    expect(githubSchedule({ ...due, inFlight: true })).toBe("none");
  });
  it("recounts locally when the head moved", () => {
    expect(githubSchedule({ ...base, headMoved: true, now: 1 })).toBe("local");
    expect(githubSchedule({ ...base, headMoved: true, now: 30_000 })).toBe("ask");
  });
});
describe("recountKey", () => {
  it("changes with the branch when the commit stays the same", () => {
    expect(recountKey({ branch: "feat", head: "abc" })).not.toBe(recountKey({ branch: "other", head: "abc" }));
  });
  it("changes with the commit, and is null with nothing listed", () => {
    expect(recountKey({ branch: "feat", head: "abc" })).not.toBe(recountKey({ branch: "feat", head: "abd" }));
    expect(recountKey({ branch: "feat", head: "abc" })).toBe(recountKey({ branch: "feat", head: "abc" }));
    expect(recountKey(null)).toBeNull();
    expect(recountKey({ branch: null, head: null })).not.toBeNull();
  });
});

describe("askedAt", () => {
  it("moves the time GitHub was last asked only for an ask, not for a local recount", () => {
    expect(askedAt("ask", 1_000, 9_000)).toBe(9_000);
    expect(askedAt("local", 1_000, 9_000)).toBe(1_000);
  });
});
