import { describe, expect, it } from "vitest";
import { githubSchedule } from "./useGithub";

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
