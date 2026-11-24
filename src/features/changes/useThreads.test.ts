import { describe, expect, it } from "vitest";
import type { GithubReply } from "../../generated/GithubReply";
import type { PullState } from "../../generated/PullState";
import { sameRequest, takesAnswer, threadsTarget } from "./useThreads";

const reply = (state: PullState | null, over: Partial<GithubReply> = {}): GithubReply =>
  ({
    kind: "ready",
    checkout: "C:/r",
    host: "github.com",
    repo: "o/r",
    branch: "feat",
    pull: state === null ? null : { id: "PR_1", number: 7, title: "t", url: "https://github.com/o/r/pull/7", state, base: "main", head: "feat", readiness: "ready", review: "none", checks: [], more_checks: 0, auto_merge: null, can_auto_merge: false },
    ...over,
  }) as GithubReply;

describe("threadsTarget", () => {
  it("threadsTarget names an open or a draft pull request and nothing else", () => {
    expect(threadsTarget(reply("open"))).toEqual({ checkout: "C:/r", id: "PR_1", number: 7 });
    expect(threadsTarget(reply("draft"))).toEqual({ checkout: "C:/r", id: "PR_1", number: 7 });
    expect(threadsTarget(reply("merged"))).toBeNull();
    expect(threadsTarget(reply("closed"))).toBeNull();
    expect(threadsTarget(reply(null))).toBeNull();
    expect(threadsTarget(reply("open", { kind: "unreachable" }))).toBeNull();
    expect(threadsTarget(null)).toBeNull();
  });
});

describe("sameRequest", () => {
  it("compares the checkout, the node id and the number", () => {
    const a = { checkout: "C:/r", id: "PR_1", number: 7 };
    expect(sameRequest(a, { ...a })).toBe(true);
    expect(sameRequest(a, { ...a, number: 8 })).toBe(false);
    expect(sameRequest(a, { ...a, id: "PR_2" })).toBe(false);
    expect(sameRequest(a, { ...a, checkout: "C:/other" })).toBe(false);
    expect(sameRequest(a, null)).toBe(false);
  });
});

describe("takesAnswer", () => {
  const target = { checkout: "C:/r", id: "PR_1", number: 7 };
  it("takes the answer of the latest ask for the pull request of now", () => {
    expect(takesAnswer({ started: 3, latest: 3, asked: target, target: { ...target } })).toBe(true);
  });
  it("drops one that started before the last refresh or ask", () => {
    expect(takesAnswer({ started: 2, latest: 3, asked: target, target })).toBe(false);
  });
  it("drops one for another pull request, and any with no target", () => {
    expect(takesAnswer({ started: 3, latest: 3, asked: target, target: { ...target, number: 8 } })).toBe(false);
    expect(takesAnswer({ started: 3, latest: 3, asked: target, target: null })).toBe(false);
  });
});
