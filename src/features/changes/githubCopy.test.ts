import { describe, expect, it } from "vitest";
import { githubCopy as c, METHOD_BUTTONS, METHOD_LABELS } from "./githubCopy";

describe("githubCopy", () => {
  it("agrees in number", () => {
    expect(c.toPush(1)).toBe("1 commit to push");
    expect(c.toPush(3)).toBe("3 commits to push");
    expect(c.pushButton(1)).toBe("Push 1 commit");
    expect(c.pushButton(2)).toBe("Push 2 commits");
    expect(c.notPushedYet(1)).toBe("1 commit isn't on GitHub yet. Push it first.");
    expect(c.notPushedYet(2)).toBe("2 commits aren't on GitHub yet. Push them first.");
  });
  it("words the methods", () => {
    expect(METHOD_LABELS.merge).toBe("Create a merge commit");
    expect(METHOD_BUTTONS.merge).toBe("Merge");
    expect(METHOD_BUTTONS.squash).toBe("Squash and merge");
  });
  it("uses the decided sentences", () => {
    expect(c.rateLimited("3:05 PM")).toBe("GitHub's limit for this sign-in is used up. Marshell asks again at 3:05 PM.");
    expect(c.conflicts(182, "main")).toBe("#182 has conflicts with main. Resolve them in the terminal.");
    expect(c.created(5)).toBe("Pull request #5 was created.");
  });
  it("words the checks list and the bar's own actions", () => {
    expect(c.andMore(3)).toBe("And 3 more on GitHub.");
    expect(c.askFix("Claude")).toBe("Ask Claude to fix");
    expect(c.checkStates.failed).toBe("Failed");
    expect(c.hidden).toBe("Pull requests are hidden. Settings → Changes shows them again.");
    expect(c.typedRequest("api")).toBe("Typed a request into api.");
  });
});
