import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { TrayMenu } from "./TrayMenu";
import type { TrayState } from "./trayItems";

const wait = (n: number, name: string, status: "needs-permission" | "needs-question" = "needs-permission") => ({ number: n, name, status, phrase: status === "needs-permission" ? "Wants to run `npm test`" : "Has a question", agent: "claude" as const });
const state: TrayState = { os: "windows", sessions: [wait(3, "api-server"), wait(2, "infra", "needs-question")], paused: false, doctorIssues: 4 };
const html = (s: Partial<TrayState> = {}, caption = false) => renderToStaticMarkup(<TrayMenu state={{ ...state, ...s }} caption={caption} />);

describe("TrayMenu", () => {
  it("is a menu of menu items, with separators and a disabled line done the ARIA way", () => {
    const h = html();
    expect(h).toContain('role="menu"');
    expect(h).toMatch(/role="menuitem"/);
    expect(h).toContain('role="separator"');
    expect(html({ sessions: [] })).toMatch(/aria-disabled="true"[^>]*>[\s\S]*?Nothing needs you/);
  });
  it("shows a waiting session with its number and its mark", () => {
    const h = html();
    expect(h).toContain("session-number");
    expect(h).toContain("agent-mark");
    expect(h).toContain("api-server");
  });
  it("names every item from its visible text", () => {
    for (const item of html().match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(item)).toBe(true);
  });
  it("lists Open Departures and shows a shortcut only where the command has one", () => {
    const h = html();
    expect(h).toContain("Open Departures");
    expect(h).toMatch(/<kbd[^>]*>Ctrl\+Shift\+N<\/kbd>/);
  });
  it("says on Linux that the tray is menu-only, and marks macOS and Linux as design only", () => {
    expect(html({ os: "linux" }, true)).toContain("On Linux the tray is menu-only: it has no click events.");
    expect(html({ os: "linux" }, true)).toContain("Design only");
    expect(html({ os: "mac" }, true)).toContain("Design only");
    expect(html({ os: "windows" }, true)).not.toContain("Design only");
    expect(html({ os: "windows" }, true)).not.toContain("menu-only");
  });
  it("carries no live region of its own", () => {
    expect(html()).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
});
