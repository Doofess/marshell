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
    expect(c.copiedRequest("api")).toBe("Copied the request. api has no prompt line.");
    expect(c.copiedCommands).toBe("Copied the two commands.");
    expect(c.pullsOff).toBe("Pull requests from GitHub are off in Settings.");
    expect(c.noPullHere).toBe("This branch has no pull request.");
    expect(c.unavailable).toBe("Pull requests aren't available for this checkout.");
  });
  it("words the sheets", () => {
    expect(c.willPush(1)).toBe("1 commit will be pushed to origin.");
    expect(c.willPush(3)).toBe("3 commits will be pushed to origin.");
    expect(c.uncommitted(1)).toBe("1 file isn't committed and stays out.");
    expect(c.uncommitted(2)).toBe("2 files aren't committed and stay out.");
    expect(c.pushed(2)).toBe("Pushed 2 commits to origin.");
    expect(c.pushed(1)).toBe("Pushed 1 commit to origin.");
    expect(c.pushed(null)).toBe("Pushed to origin.");
    expect(c.headLine("feat", "main", "octo-org/widgets")).toBe("feat → main · octo-org/widgets");
    expect(c.mergeLine("Squash and merge", "Fix", 182, "main")).toBe('Squash and merge "Fix" (#182) into main.');
    expect(c.mergeTitle(182)).toBe("Merge #182?");
    expect(c.mergesLater(182)).toBe("#182 merges when its checks pass.");
    expect(c.pushingTo("feat")).toBe("Pushing feat to origin…");
  });
});
