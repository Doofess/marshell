import { describe, expect, it } from "vitest";
import { focusOwnerOf, ownerKeyOf, ownerPageKey, PANE_AREA, paneOwner, renamed, SESSION_AREA, SHELL_DELIVERY, SHELL_PANEL_AREA } from "./paneOwner";

const within = (...areas: string[]) => ({ closest: (s: string) => (areas.some((a) => s === a) ? ({} as Element) : null) });

describe("paneOwner", () => {
  it("no session gives the shell", () => {
    expect(paneOwner({ pref: "session", focusedId: undefined, shellShown: false, panelTabId: "t9" })).toEqual({ key: "shell", tabId: "t9", shell: true });
  });
  it("focus in the panel on screen gives the shell", () => {
    expect(paneOwner({ pref: "shell", focusedId: "s1", shellShown: true, panelTabId: "t9" })).toEqual({ key: "shell", tabId: "t9", shell: true });
  });
  it("a panel off screen gives the session", () => {
    expect(paneOwner({ pref: "shell", focusedId: "s1", shellShown: false, panelTabId: "t9" })).toEqual({ key: "s1", tabId: "s1", shell: false });
  });
  it("a shell not there yet has no tab id", () => {
    expect(paneOwner({ pref: "session", focusedId: undefined, shellShown: false, panelTabId: undefined }).tabId).toBeNull();
  });
});

describe("focusOwnerOf", () => {
  it("the shell panel is the shell", () => {
    expect(focusOwnerOf(within(SHELL_PANEL_AREA, SESSION_AREA))).toBe("shell");
  });
  it("a session's terminal is the session", () => {
    expect(focusOwnerOf(within(SESSION_AREA))).toBe("session");
  });
  it("a sidebar row is the session", () => {
    expect(focusOwnerOf(within(SESSION_AREA))).toBe("session");
  });
  it("the pane leaves it", () => {
    expect(focusOwnerOf(within(PANE_AREA, SESSION_AREA))).toBeNull();
  });
  it("the title bar leaves it", () => {
    expect(focusOwnerOf(within())).toBeNull();
  });
  it("nothing focused leaves it", () => {
    expect(focusOwnerOf(null)).toBeNull();
  });
});

describe("keys", () => {
  it("ownerKeyOf", () => {
    expect(ownerKeyOf("t9", "t9")).toBe("shell");
    expect(ownerKeyOf("s1", "t9")).toBe("s1");
    expect(ownerKeyOf("t9", undefined)).toBe("t9");
  });
  it("ownerPageKey", () => {
    expect(ownerPageKey("shell", "p1", "t9")).toBe("t9/p1");
    expect(ownerPageKey("s1", "p1", "t9")).toBe("s1/p1");
    expect(ownerPageKey("shell", "p1", undefined)).toBe("shell/p1");
  });
  it("renamed", () => {
    expect(renamed(new Set(["shell", "s1"]), "shell", "t9")).toEqual(new Set(["t9", "s1"]));
    const s = new Set(["s1"]);
    expect(renamed(s, "shell", "t9")).toBe(s);
  });
  it("SHELL_DELIVERY", () => {
    expect(SHELL_DELIVERY).toEqual({ name: "The shell panel", via: "clipboard" });
  });
});
