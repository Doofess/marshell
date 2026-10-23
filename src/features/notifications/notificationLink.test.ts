import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { focusTarget, linkFocusSelector, readNotificationLink, setNotificationLink } from "./notificationLink";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => vi.clearAllMocks());

describe("focusTarget", () => {
  it("names the tab when it is still open", () => {
    expect(focusTarget("t1", ["t1", "t2"])).toBe("t1");
  });
  it.each([
    ["a closed tab", "gone", ["t1"]],
    ["no tab", null, ["t1"]],
    ["no open tabs", "t1", []],
  ])("gives null for %s", (_n, id, ids) => {
    expect(focusTarget(id as string | null, ids)).toBeNull();
  });
});

describe("linkFocusSelector", () => {
  const rows = [
    { id: "t1", number: 1 },
    { id: "t2", number: 2 },
  ];
  it("puts focus on the linked session's row, never in its terminal", () => {
    expect(linkFocusSelector("t2", "t1", rows)).toBe('[data-nav-row="2"]');
  });
  it("uses the selected session's row when the link names none (show, or a closed session)", () => {
    expect(linkFocusSelector(null, "t1", rows)).toBe('[data-nav-row="1"]');
  });
  it("falls back to the main area with no session to name", () => {
    expect(linkFocusSelector(null, null, rows)).toBe(".window__main");
    expect(linkFocusSelector("gone", null, [])).toBe(".window__main");
  });
});

describe("readNotificationLink", () => {
  it("resolves what the command returns", async () => {
    vi.mocked(invoke).mockResolvedValue(true);
    expect(await readNotificationLink()).toBe(true);
    expect(invoke).toHaveBeenCalledWith("notification_link");
  });
  it("resolves null when the command returns null", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    expect(await readNotificationLink()).toBeNull();
  });
  it("resolves null when the invoke rejects", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("no tauri"));
    expect(await readNotificationLink()).toBeNull();
  });
});

describe("setNotificationLink", () => {
  it("passes the choice and returns the state the command reports", async () => {
    vi.mocked(invoke).mockResolvedValue(true);
    expect(await setNotificationLink(true)).toBe(true);
    expect(invoke).toHaveBeenCalledWith("set_notification_link", { on: true });
  });
  it("lets a failure through for the dialog to show", async () => {
    vi.mocked(invoke).mockRejectedValue("denied");
    await expect(setNotificationLink(false)).rejects.toBe("denied");
  });
});
