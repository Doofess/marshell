import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MainWindow } from "./MainWindow";
import { SCENARIO_ROWS } from "./scenario";

const BIG = { width: 1700, height: 900 } as const;
const html = (p: Partial<Parameters<typeof MainWindow>[0]> = {}) => renderToStaticMarkup(<MainWindow width={1180} height={800} selectedId="working" {...p} />);

describe("MainWindow: default", () => {
  const h = html();
  it("has one unified 40 px top bar, a sidebar and one terminal", () => {
    expect(h).toContain("window__bar");
    expect(h).toContain("session-header");
    expect(h).toContain('class="sidebar"');
    expect((h.match(/class="scripted-terminal"/g) ?? []).length).toBe(1);
  });
  it("shows the lane, with the oldest request expanded", () => {
    expect(h).toContain("Needs you · 2");
    expect(h).toContain("Which AWS region");
  });
  it("has Windows window controls and a palette button in the bar", () => {
    for (const s of ["Minimize", "Maximize", "Close", "Command palette"]) expect(h, s).toContain(`aria-label="${s}"`);
  });
  it("shows the session the user selected in the header", () => {
    expect(h).toContain('aria-label="Session api-server"');
  });
});

describe("MainWindow: layouts", () => {
  it("collapses to the rail below 960 px", () => {
    const h = html({ width: 940 });
    expect(h).toContain('class="rail"');
    expect(h).not.toContain('class="sidebar"');
  });
  it("pins the rail when asked, at any width", () => {
    expect(html({ sidebarPref: "rail" })).toContain('class="rail"');
  });
  it("is 720 by 480 at the minimum, still showing a terminal and the header", () => {
    const h = html({ width: 720, height: 480 });
    expect(h).toContain("inline-size:720px");
    expect(h).toContain("scripted-terminal");
    expect(h).toContain("session-header");
  });
  it("focus mode hides the sidebar and header, and floats a pill naming the shortcut", () => {
    const h = html({ layout: "focus", sidebarPref: "focus" });
    expect(h).not.toContain('class="sidebar"');
    expect(h).not.toContain("session-header");
    expect(h).toContain("focus-pill");
    expect(h).toContain("2 need you");
    expect(h).toContain("Ctrl+Shift+N");
  });
  it("focus mode can show the header as the top edge is hovered", () => {
    expect(html({ layout: "focus", sidebarPref: "focus", headerPeek: true })).toContain("session-header");
  });
  it("uses the Mac shortcut label on macOS", () => {
    expect(html({ layout: "focus", sidebarPref: "focus", os: "mac" })).toContain("⌘N");
  });
  it("split view has two panes, a separator and exactly one active pane", () => {
    const h = html({ layout: "split" });
    expect((h.match(/class="scripted-terminal"/g) ?? []).length).toBe(2);
    expect(h).toContain('role="separator"');
    expect((h.match(/data-active="true"/g) ?? []).length).toBe(1);
    expect(h).toContain('data-active="false"');
  });
  it("the drawer shows the first request in full and pushes or overlays by width", () => {
    expect(html({ drawer: true, width: 1700 })).toContain('data-placement="push"');
    expect(html({ drawer: true, width: 1180 })).toContain('data-placement="overlay"');
  });
  it("passes the terminal setting through", () => {
    expect(html({ terminalSetting: "light" })).toContain('data-setting="light"');
  });
});

describe("MainWindow: logo and accent", () => {
  it("shows the app logo and its name in the bar, as the app menu button named for what it shows", () => {
    const h = html();
    expect(h).toMatch(/<button[^>]*aria-label="Marshell menu"[^>]*aria-haspopup="menu"[^>]*><svg class="logo"[\s\S]*?<svg class="wordmark"/);
  });
  it("drops the name when the sidebar is the rail, keeping the icon", () => {
    const h = html({ sidebarPref: "rail" });
    expect(h).toContain('class="logo"');
    expect(h).not.toContain('class="wordmark"');
  });
  it("hides the logo's own name inside that button, so it is not announced twice", () => {
    expect(html()).not.toContain('aria-label="Marshell"');
  });
  it("keeps the logo when the sidebar is the rail, where the 52 px column has room for only it", () => {
    const h = html({ sidebarPref: "rail" });
    expect(h).toContain('class="logo"');
    expect(h.slice(h.indexOf("window__bar-side"), h.indexOf("window__bar-main"))).not.toContain("Command palette");
  });
  it("moves the palette button to the bottom of the rail so it is never lost", () => {
    const h = html({ sidebarPref: "rail" });
    expect(h.slice(h.indexOf('class="rail"'))).toContain('aria-label="Command palette"');
  });
  it("sets the accent for the whole window to the selected session's vendor", () => {
    expect(html()).toMatch(/class="window"[^>]*data-agent="claude"/);
    const codex = SCENARIO_ROWS.map((r) => (r.id === "working" ? { ...r, agent: "codex" as const } : r));
    expect(html({ rows: codex })).toMatch(/class="window"[^>]*data-agent="codex"/);
  });
});

describe("MainWindow: the palette button says what it is", () => {
  it("is a labelled button with a magnifier and the words Command palette, not a bare plus", () => {
    const h = html();
    expect(h).toMatch(/class="window__palette"[^>]*aria-label="Command palette"[\s\S]*?data-icon="search"[\s\S]*?<span>Command palette<\/span>/);
    expect(h).not.toContain("M1 5h8M5 1v8");
  });
  it("names what it opens and the shortcut in its tooltip", () => {
    expect(html()).toContain("Search sessions, projects and commands (Ctrl+Shift+K)");
    expect(html({ os: "mac" })).toContain("(⌘K)");
  });
  it("uses the same magnifier in the rail, where there is room for the icon only", () => {
    const h = html({ sidebarPref: "rail" });
    expect(h.slice(h.indexOf('class="rail"'))).toContain('data-icon="search"');
    expect(h).not.toContain("M2.5 4.5L6 7l-3.5 2.5M7.5 10h4");
  });
});

describe("MainWindow: split view up to four", () => {
  const panes = (h: string) => (h.match(/<section class="pane"/g) ?? []).length;
  it("shows two panes by default and up to four, in a window big enough for them", () => {
    expect(panes(html({ layout: "split", ...BIG }))).toBe(2);
    expect(panes(html({ layout: "split", splitCount: 3, ...BIG }))).toBe(3);
    expect(panes(html({ layout: "split", splitCount: 4, ...BIG }))).toBe(4);
  });
  it("shows fewer than asked for when the window is too small to make them useful", () => {
    expect(panes(html({ layout: "split", splitCount: 4, width: 720, height: 480 }))).toBe(1);
    expect(panes(html({ layout: "split", splitCount: 3, width: 1180, height: 800 }))).toBe(2);
    expect(panes(html({ layout: "split", splitCount: 4, width: 1500, height: 860 }))).toBe(4);
  });
  it("says in the header of the focused pane how many more are running out of sight", () => {
    expect(html({ layout: "split", splitCount: 4, width: 720, height: 480 })).toContain("3 more running");
    expect(html({ layout: "split", splitCount: 3, width: 1180, height: 800 })).toContain("1 more running");
  });
  it("counts the prompt bar's height when it decides what fits", () => {
    // A 2 x 2 grid needs 565 px under the bar; at 650 px tall the 94 px prompt bar leaves 516, so four do not fit until it is off.
    expect(panes(html({ layout: "split", splitCount: 4, width: 1500, height: 650 }))).toBeLessThan(4);
    expect(panes(html({ layout: "split", splitCount: 4, width: 1500, height: 650, composer: false }))).toBe(4);
  });
  it("starts with the selected session and fills the rest from the others in order, never twice", () => {
    const h = html({ layout: "split", splitCount: 4, selectedId: "working", width: 1700, height: 900 });
    expect(h.indexOf("Terminal: api-server")).toBeLessThan(h.indexOf("Terminal: billing"));
    const names = [...h.matchAll(/aria-label="Terminal: ([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(names).size).toBe(4);
  });
  it("sets the window accent from the active pane", () => {
    expect(html({ layout: "split", splitCount: 2, activePane: 1 })).toMatch(/class="window"[^>]*data-agent="claude"/);
  });
  it("chooses the arrangement from the space the panes share, after the sidebar and bar", () => {
    expect(html({ layout: "split", splitCount: 4, width: 1700, height: 900 })).toContain('data-arrangement="grid"');
    expect(html({ layout: "split", splitCount: 2, width: 1700, height: 900 })).toContain('data-arrangement="columns"');
  });
  it("zooms one pane to fill the view", () => {
    const h = html({ layout: "split", splitCount: 4, zoomedPane: 1, width: 1700, height: 900 });
    expect(panes(h)).toBe(1);
    expect(h).toContain("3 more running");
  });
  it("offers an empty pane the sessions that are not on screen", () => {
    const h = html({ layout: "split", splitCount: 3, emptySlot: true, ...BIG });
    expect(h).toContain("Choose a session for this pane");
    expect(h).toContain("pane--empty");
  });
});

describe("MainWindow: layout button", () => {
  it("is in the bar, named, and closed by default", () => {
    const h = html({ layout: "split" });
    expect(h).toMatch(/aria-label="Split layout"[^>]*aria-haspopup="dialog"[^>]*aria-expanded="false"/);
    expect(h).not.toContain('role="dialog"');
  });
  it("opens the layout menu with the current count and arrangement", () => {
    const h = html({ layout: "split", splitCount: 4, layoutMenu: true, ...BIG });
    expect(h).toContain('aria-expanded="true"');
    expect(h).toContain('role="dialog"');
    expect(h).toMatch(/aria-checked="true"[^>]*aria-label="4 panes"/);
  });
  it("tells the menu what the window can hold, so the counts it cannot are off", () => {
    const h = html({ layout: "split", splitCount: 4, layoutMenu: true, width: 1180, height: 800 });
    expect(h).toContain("This window fits 2");
    expect(h).toContain("Showing 2 of 4");
    expect(h).toMatch(/aria-disabled="true"[^>]*aria-label="3 panes"/);
  });
  it("is there in a single-pane window too, since that is where a user first splits", () => {
    expect(html()).toContain('aria-label="Split layout"');
  });
});

describe("MainWindow: the prompt bar", () => {
  it("sits under the terminal in every layout but focus mode, where the terminal has the whole window", () => {
    for (const layout of ["default", "split", "rail"] as const) expect(html({ layout }), layout).toContain('class="composer"');
    expect(html({ layout: "focus", sidebarPref: "focus" })).not.toContain('class="composer"');
    expect(html({ composer: false })).not.toContain('class="composer"');
  });
  it("writes to the session of the single terminal", () => {
    expect(html()).toContain('aria-label="Message api-server"');
  });
  it("writes to the focused pane in split view, and names it", () => {
    const first = html({ layout: "split", splitCount: 2, width: 1700, height: 900, activePane: 0 });
    expect(first).toContain('aria-label="Message api-server"');
    expect(first).toContain("Pane 1 of 2");
    const second = html({ layout: "split", splitCount: 2, width: 1700, height: 900, activePane: 1 });
    expect(second).toContain('aria-label="Message billing"');
    expect(second).toContain("Pane 2 of 2");
  });
  it("takes the accent from the focused pane's session, matching the pane's bar", () => {
    const h = html({ layout: "split", splitCount: 4, width: 1700, height: 900, activePane: 3 });
    const agentOfLastPane = /Terminal: ([^"]+)"/g;
    const names = [...h.matchAll(agentOfLastPane)].map((m) => m[1]);
    expect(h).toContain(`aria-label="Message ${names[3]}"`);
  });
  it("is off, with the reason, when the focused pane is empty", () => {
    const h = html({ layout: "split", splitCount: 3, emptySlot: true, activePane: 2, width: 1700, height: 900 });
    expect(h).toContain("Choose a session for this pane first");
  });
  it("shows the text it was given", () => {
    expect(html({ composerValue: "Add retry tests" })).toContain("Add retry tests");
  });
});

describe("MainWindow: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    for (const layout of ["default", "split", "focus", "rail"] as const)
      expect(html({ layout, sidebarPref: layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded" })).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});

describe("MainWindow (accessibility gate)", () => {
  it("gives the focusable split separator the values a separator needs", () => {
    const h = html({ layout: "split" });
    expect(h).toContain('role="separator"');
    expect(h).toContain('aria-valuenow="50"');
    expect(h).toContain('aria-valuemin="20"');
    expect(h).toContain('aria-valuemax="80"');
  });
  it("names the focus pill with the text it shows", () => {
    const h = html({ layout: "focus", sidebarPref: "focus" });
    expect(h).toContain('aria-label="2 need you Ctrl+Shift+N"');
    expect(h).toMatch(/<span>2 need you<\/span> <kbd/);
  });
});
