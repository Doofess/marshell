import { describe, expect, it } from "vitest";
import { BANNER_COMMAND_MAX, batchBanner, bannerFor, DONE_MIN_BACKGROUND_MS, escalationBanner, type NotifyEvent } from "./banner";

const perm: NotifyEvent = { kind: "needs-permission", number: 3, name: "api-server", project: "my-app", detail: "npm test" };
describe("bannerFor: what it says", () => {
  it("a permission request names the session and the command and offers Allow and Deny where the OS can", () => {
    const b = bannerFor(perm, "windows")!;
    expect(b.title).toBe("api-server needs you");
    expect(b.body).toBe("Wants to run `npm test`");
    expect(b.meta).toBe("Session 3 · my-app");
    expect(b.actions).toEqual(["Allow", "Deny"]);
    expect(bannerFor(perm, "mac")!.actions).toEqual(["Allow", "Deny"]);
    expect(bannerFor(perm, "linux")!.actions).toEqual(["Open"]);
  });
  it("a risky request shows only Review and never the command", () => {
    for (const os of ["windows", "mac", "linux"] as const) {
      const b = bannerFor({ ...perm, risky: true, detail: "rm -rf node_modules" }, os)!;
      expect(b.actions).toEqual(["Review"]);
      expect(`${b.title} ${b.body}`).not.toContain("rm -rf");
      expect(b.body).toBe("Needs a careful look.");
    }
  });
  it("a question says so and opens the session", () => {
    const b = bannerFor({ kind: "needs-question", number: 2, name: "infra", project: "terraform" }, "windows")!;
    expect(b.body).toBe("Has a question");
    expect(b.actions).toEqual(["Open"]);
  });
  it("an error says why and when it resets", () => {
    const b = bannerFor({ kind: "error", number: 6, name: "scratch", detail: "rate limit reached", resetsAt: "3:40 pm" }, "windows")!;
    expect(b.title).toBe("scratch stopped");
    expect(b.body).toBe("Stopped: rate limit reached. Resets at 3:40 pm.");
  });
});

describe("bannerFor: Allow only when the whole command is shown", () => {
  it("has a cap per system: 160 characters on Windows and Linux, 120 on macOS", () => {
    expect(BANNER_COMMAND_MAX).toEqual({ windows: 160, mac: 120, linux: 160 });
  });
  it("offers Allow and Deny only for a command that fits on one line", () => {
    expect(bannerFor({ ...perm, detail: "x".repeat(160) }, "windows")!.actions).toEqual(["Allow", "Deny"]);
    expect(bannerFor({ ...perm, detail: "x".repeat(120) }, "mac")!.actions).toEqual(["Allow", "Deny"]);
  });
  it("offers only Open for a command over the cap, so a cut-short command cannot be allowed", () => {
    const b = bannerFor({ ...perm, detail: "x".repeat(161) }, "windows")!;
    expect(b.actions).toEqual(["Open"]);
    expect(b.body).not.toContain("xxxxx");
  });
  it("a 121-character command is over the cap on macOS and within it on Windows", () => {
    const cmd = "x".repeat(121);
    expect(bannerFor({ ...perm, detail: cmd }, "mac")!.actions).toEqual(["Open"]);
    expect(bannerFor({ ...perm, detail: cmd }, "mac")!.body).toBe("Wants to run a command that does not fit here.");
    expect(bannerFor({ ...perm, detail: cmd }, "windows")!.actions).toEqual(["Allow", "Deny"]);
  });
  it("Linux shows a command up to 160 characters in the body, with Open as the only action either way", () => {
    expect(bannerFor({ ...perm, detail: "x".repeat(160) }, "linux")!.body).toContain("xxxxx");
    expect(bannerFor({ ...perm, detail: "x".repeat(161) }, "linux")!.body).toBe("Wants to run a command that does not fit here.");
    expect(bannerFor({ ...perm, detail: "x".repeat(161) }, "linux")!.actions).toEqual(["Open"]);
  });
  it("offers only Open for a multi-line command", () => {
    expect(bannerFor({ ...perm, detail: "cd app\nrm file" }, "mac")!.actions).toEqual(["Open"]);
  });
  it("offers only Open when the command is not known", () => {
    expect(bannerFor({ ...perm, detail: undefined }, "windows")!.actions).toEqual(["Open"]);
  });
});

describe("bannerFor: when it stays quiet", () => {
  it("is silent while the window is focused", () => {
    expect(bannerFor({ ...perm, windowFocused: true }, "windows")).toBeNull();
  });
  it("never notifies for a stuck session or a finished subagent", () => {
    expect(bannerFor({ kind: "stuck", number: 1, name: "a" }, "windows")).toBeNull();
    expect(bannerFor({ kind: "subagent-done", number: 1, name: "a" }, "windows")).toBeNull();
  });
  it("says done only for a session that has been in the background for more than 30 seconds", () => {
    expect(DONE_MIN_BACKGROUND_MS).toBe(30_000);
    expect(bannerFor({ kind: "done", number: 5, name: "docs", backgroundMs: 29_000 }, "windows")).toBeNull();
    expect(bannerFor({ kind: "done", number: 5, name: "docs", backgroundMs: 31_000 }, "windows")!.title).toBe("docs is done");
  });
});

describe("bannerFor: what every banner promises", () => {
  it("is silent (Marshell plays its own sound), never raises the window, and focuses the session when clicked", () => {
    for (const os of ["windows", "mac", "linux"] as const) {
      const b = bannerFor(perm, os)!;
      expect(b.silent).toBe(true);
      expect(b.raisesWindow).toBe(false);
      expect(b.activation).toBe("focus-session");
      expect(b.sessionNumber).toBe(3);
    }
  });
  it("follows the copy rules", () => {
    const all = [bannerFor(perm, "windows")!, bannerFor({ ...perm, risky: true }, "windows")!, bannerFor({ kind: "error", number: 1, name: "x", detail: "rate limit reached" }, "windows")!];
    for (const b of all) expect(`${b.title} ${b.body}`).not.toMatch(/!|successfully|oops|the ai/i);
  });
});

describe("batching and escalation", () => {
  const q: NotifyEvent = { kind: "needs-question", number: 2, name: "infra" };
  const p2: NotifyEvent = { kind: "needs-permission", number: 1, name: "billing", detail: "npm test" };
  it("folds several requests within 3 seconds into one banner", () => {
    const b = batchBanner([perm, q, p2], "windows")!;
    expect(b.title).toBe("3 sessions need you");
    expect(b.body).toBe("Sessions 3 api-server, 2 infra and 1 billing");
    expect(b.actions).toEqual(["Open Marshell"]);
  });
  it("leads each session with its number, as everywhere else, so a number is never read as a count", () => {
    const b = batchBanner([perm, q], "windows")!;
    expect(b.body).toMatch(/^Sessions 3 api-server and 2 infra$/);
    expect(b.body).not.toContain("(3)");
  });
  it("shows only Review when any of them is risky", () => {
    expect(batchBanner([perm, { ...p2, risky: true }], "windows")!.actions).toEqual(["Review"]);
  });
  it("is the ordinary banner for one event and nothing for none", () => {
    expect(batchBanner([perm], "windows")).toEqual(bannerFor(perm, "windows"));
    expect(batchBanner([], "windows")).toBeNull();
  });
  it("escalates once with the wait in minutes", () => {
    expect(escalationBanner({ ...perm, waitedMs: 60_000 }, "windows").body).toBe("api-server has waited 1 min to run a command");
  });
  it("an escalation never offers Allow or Deny: its body no longer shows the command", () => {
    for (const os of ["windows", "mac", "linux"] as const) {
      expect(escalationBanner({ ...perm, waitedMs: 120_000 }, os).actions).toEqual(["Open"]);
      expect(escalationBanner({ ...perm, risky: true, waitedMs: 120_000 }, os).actions).toEqual(["Review"]);
    }
  });
});
