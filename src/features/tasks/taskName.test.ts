import { describe, expect, it } from "vitest";
import { branchHint, slugify, taskNameFor } from "./taskName";

describe("slugify (the same rules and vectors as the core's slug)", () => {
  it("lowercases ASCII and turns every other run into one dash", () => {
    expect(slugify("Fix the login redirect, please!")).toBe("fix-the-login-redirect-please");
    expect(slugify("  --A__b  ")).toBe("a-b");
  });
  it("gives nothing for text with no ASCII letters or digits; the core then picks task-<n>", () => {
    expect(slugify("תקן את הכפתור 🙂")).toBe("");
    expect(slugify("🙂")).toBe("");
    expect(slugify("")).toBe("");
  });
  it("cuts at the last dash at or before 40 characters, with no trailing dash", () => {
    const s = slugify("one two three four five six seven eight nine ten eleven");
    expect(s).toBe("one-two-three-four-five-six-seven-eight");
    expect(s.length).toBeLessThanOrEqual(40);
  });
  it("hard-cuts at 40 when there is no dash to cut at", () => {
    expect(slugify("x".repeat(60))).toBe("x".repeat(40));
  });
});

describe("taskNameFor", () => {
  it("follows the prompt until the user edits the field", () => {
    expect(taskNameFor(null, "Fix the button")).toBe("fix-the-button");
    expect(taskNameFor("my-name", "Fix the button")).toBe("my-name");
  });
  it("keeps an emptied field empty instead of going back to the prompt", () => {
    expect(taskNameFor("", "Fix the button")).toBe("");
  });
});

describe("branchHint", () => {
  it("names the branch the core will make", () => {
    expect(branchHint("fix-login")).toBe("Branch marshell/fix-login");
    expect(branchHint("Fix Login!")).toBe("Branch marshell/fix-login");
  });
  it("says task-… while the name is empty", () => {
    expect(branchHint("")).toBe("Branch marshell/task-…");
    expect(branchHint("🙂")).toBe("Branch marshell/task-…");
  });
});
