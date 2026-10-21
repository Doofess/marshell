import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MainWindow } from "./MainWindow";
import { COMPOSER, STATUS_BAR_HEIGHT } from "./layout";
import { SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { windowTitle } from "./windowTitle";
import type { SessionInfo } from "../../generated/SessionInfo";
import { offersModeChange } from "../sessions/resumeState";

const BIG = { width: 1700, height: 900 } as const;
const html = (p: Partial<Parameters<typeof MainWindow>[0]> = {}) => renderToStaticMarkup(<MainWindow width={1180} height={800} selectedId="working" {...p} />);

const NUM: Record<string, number> = { "api-server": 3, billing: 1, infra: 2, landing: 5, search: 6 };

describe("MainWindow: default", () => {
  it("keeps the ended agent's mode as text while its shell runs, with no button semantics or tab stop", () => {
    const row = { ...SCENARIO_ROWS[0]!, status: "ended" as const, mode: "plan" as const };
    const render = (state: SessionInfo["state"]) => {
      const info = { state } as SessionInfo;
      const h = html({ rows: [row], selectedId: row.id, onModeMenu: () => {}, canChangeMode: (r) => offersModeChange(r, info) });
      return h.match(/<div class="session-header".*?<\/div>/)![0];
    };
    const h = render({ kind: "running" });
    expect(h).toMatch(/<span[^>]*class="chip"[^>]*>Plan<\/span>/);
    expect(h).not.toContain("<button");
    expect(h).not.toContain('role="button"');
    expect(h).not.toContain("tabindex");
    expect(h).not.toContain("aria-haspopup");
    expect(render({ kind: "exited", code: 0 })).toMatch(/<button[^>]*class="session-header__mode"/);
  });
  const h = html();
  it("has one unified 40 px top bar, a sidebar and one terminal", () => {
    expect(h).toContain("window__bar");
    expect(h).toContain("session-header");
    expect(h).toContain('class="sidebar"');
    expect((h.match(/class="scripted-terminal" role="group" aria-label="Terminal for/g) ?? []).length).toBe(1);
  });
  it("shows the lane, with the oldest request expanded", () => {
    expect(h).toContain("Needs you · 2");
    expect(h).toContain("Which AWS region");
  });
  it("has Windows window controls and a palette button in the bar", () => {
    for (const s of ["Minimize", "Maximize", "Close", "Command palette"]) expect(h, s).toContain(`aria-label="${s}"`);
  });
  it("shows the session the user selected in the header", () => {
    expect(h).toContain('aria-label="Session 3, api-server"');
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
    expect((h.match(/class="scripted-terminal" role="group" aria-label="Terminal for/g) ?? []).length).toBe(2);
    expect(h).toContain('role="separator"');
    expect((h.match(/data-active="true"/g) ?? []).length).toBe(1);
    expect(h).toContain('data-active="false"');
  });
  it("with two or more panes on screen, the top bar names no session: each pane's own header does", () => {
    expect(html({ layout: "split" })).not.toContain("session-header");
    expect(html({ layout: "split", splitCount: 4, ...BIG })).not.toContain("session-header");
  });
  it("keeps the top bar's session header when one pane is on screen: zoomed, or a split too small for two", () => {
    expect(html({ layout: "split", splitCount: 4, zoomedPane: 1, ...BIG })).toContain("session-header");
    expect(html({ layout: "split", splitCount: 2, width: 720, height: 480 })).toContain("session-header");
  });
  it("tints a bypass or auto session's pane header, since the top bar no longer carries the warning", () => {
    const rows = SCENARIO_ROWS.map((r) => (r.name === "billing" ? { ...r, mode: "bypass" as const } : r));
    const h = html({ layout: "split", splitCount: 2, rows, ...BIG });
    expect((h.match(/<header class="pane__header"[^>]*data-danger="true"/g) ?? []).length).toBe(1);
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
  it("keeps the logo when the sidebar is the rail, with the palette button as an icon only", () => {
    const h = html({ sidebarPref: "rail" });
    expect(h).toContain('class="logo"');
    const side = h.slice(h.indexOf("window__bar-side"), h.indexOf("window__bar-main"));
    expect(side).toContain('aria-label="Command palette"');
    expect(side).not.toContain("<span>Command palette</span>");
  });
  it("keeps the palette button in the bar, not the rail", () => {
    const h = html({ sidebarPref: "rail" });
    expect(h.slice(h.indexOf('class="rail"'))).not.toContain('aria-label="Command palette"');
  });
  it("keeps the window's own controls (the layout menu, the panel, the cursor) in the app accent: only a session's own elements take its vendor's", () => {
    expect(html()).not.toMatch(/class="window"[^>]*data-agent/);
    expect(html()).toMatch(/class="session-header"[^>]*data-agent="claude"/);
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
    expect(h.slice(h.indexOf("window__bar-side"), h.indexOf("window__bar-main"))).toContain('data-icon="search"');
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
  it("counts each pane's own prompt line when it decides what fits", () => {
    // Four panes need two stacked panes' worth of height (header, prompt line, padding and 14 terminal rows each) under the bar and the status bar.
    // At 650 px tall the window falls short with prompt lines on; with them off the terminals get those rows back.
    expect(panes(html({ layout: "split", splitCount: 4, width: 1500, height: 650 }))).toBeLessThan(4);
    expect(panes(html({ layout: "split", splitCount: 4, width: 1500, height: 650, composer: false }))).toBe(4);
  });
  it("counts the status bar's height too: the same window holds four once the bar is off", () => {
    expect(panes(html({ layout: "split", splitCount: 4, width: 1500, height: 700 }))).toBeLessThan(4);
    expect(panes(html({ layout: "split", splitCount: 4, width: 1500, height: 700, statusBar: false }))).toBe(4);
  });
  it("starts with the selected session and fills the rest from the others in order, never twice", () => {
    const h = html({ layout: "split", splitCount: 4, selectedId: "working", width: 1700, height: 900 });
    expect(h.indexOf("terminal: api-server")).toBeLessThan(h.indexOf("terminal: billing"));
    const names = [...h.matchAll(/aria-label="Session \d+, terminal: ([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(names).size).toBe(4);
  });
  it("leaves the window in the app accent with panes split: each pane carries its own vendor", () => {
    expect(html({ layout: "split", splitCount: 2, activePane: 1 })).not.toMatch(/class="window"[^>]*data-agent/);
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

describe("MainWindow: the prompt line", () => {
  const BIG = { width: 1700, height: 900 } as const;
  const forms = (h: string) => (h.match(/<form class="composer"/g) ?? []).length;
  it("is one full line under a single terminal, in a fixed slot that grows upward over the terminal", () => {
    const h = html();
    expect(forms(h)).toBe(1);
    expect(h).toContain('aria-label="Write to api-server"');
    expect(h).toMatch(/class="window__composer"[^>]*style="block-size:\d+px"/);
    expect(h).toContain(`block-size:${COMPOSER.full}px`);
  });
  it("is in the rail layout too, and gone in focus mode where the terminal has the whole window", () => {
    expect(forms(html({ layout: "rail" }))).toBe(1);
    expect(forms(html({ layout: "focus", sidebarPref: "focus" }))).toBe(0);
    expect(forms(html({ composer: false }))).toBe(0);
  });
  it("in split view is individual: every pane has its own, and there is no window-level one", () => {
    for (const n of [2, 3, 4] as const) {
      const h = html({ layout: "split", splitCount: n, ...BIG });
      expect(forms(h), String(n)).toBe(n);
      expect(h, String(n)).not.toContain("window__composer");
    }
  });
  it("names each pane's own session on its own line, so text typed in a pane goes to that pane's session", () => {
    const h = html({ layout: "split", splitCount: 4, ...BIG });
    const names = [...h.matchAll(/aria-label="Session \d+, terminal: ([^"]+)"/g)].map((m) => m[1]);
    expect(names).toHaveLength(4);
    for (const n of names) expect(h).toContain(`aria-label="Write to ${n}"`);
  });
  it("keeps the blocked state per pane: the pane whose session needs you is off while the others stay live", () => {
    const h = html({ layout: "split", splitCount: 3, ...BIG });
    const sections = h.split("</section>");
    const off = (name: string) => /<textarea[^>]*disabled/.test(sections.find((x) => x.includes(`aria-label="Session ${NUM[name]}, terminal: ${name}"`))!);
    expect(off("billing")).toBe(true);
    expect(off("api-server")).toBe(false);
  });
  it("has none in an empty pane", () => {
    const h = html({ layout: "split", splitCount: 3, emptySlot: true, ...BIG });
    expect(forms(h)).toBe(2);
    expect(h).toContain("Choose a session for this pane");
  });
  it("shows the text typed in a single window, and the text typed in each pane by session id", () => {
    expect(html({ composerValue: "Add retry tests" })).toContain("Add retry tests");
    const h = html({ layout: "split", splitCount: 2, composerValues: { working: "run the tests again" }, ...BIG });
    expect(h).toContain("run the tests again");
    expect((h.match(/run the tests again/g) ?? []).length).toBe(1);
  });
  it("is off in every pane when the window turns prompt lines off", () => {
    expect(forms(html({ layout: "split", splitCount: 4, composer: false, ...BIG }))).toBe(0);
  });
});

describe("MainWindow: the status bar follows the focused pane", () => {
  const BIG = { width: 1700, height: 900 } as const;
  const bar = (h: string) => /<div class="window__status"[^>]*>([\s\S]*?)<\/div><\/div>/.exec(h)?.[0] ?? "";
  it("is a 24 px bar at the foot of the centre column, with the window's own height from the layout constants", () => {
    const h = html();
    expect(h).toMatch(/class="window__status"[^>]*style="block-size:\d+px"/);
    expect(h).toContain(`block-size:${STATUS_BAR_HEIGHT}px`);
  });
  it("shows in the single window and in split view, and is gone in focus mode or when turned off", () => {
    expect(html()).toContain("window__status");
    expect(html({ layout: "split", ...BIG })).toContain("window__status");
    expect(html({ layout: "focus", sidebarPref: "focus" })).not.toContain("window__status");
    expect(html({ statusBar: false })).not.toContain("window__status");
  });
  it("shows the focused pane's session: its model, not the first pane's", () => {
    expect(bar(html({ layout: "split", splitCount: 2, activePane: 0, ...BIG }))).toContain("Opus 5.5");
    expect(bar(html({ layout: "split", splitCount: 2, activePane: 1, ...BIG }))).toContain("Sonnet 5.5");
  });
  it("follows a zoom: the zoomed pane is the focused one", () => {
    expect(bar(html({ layout: "split", splitCount: 4, zoomedPane: 1, activePane: 0, ...BIG }))).toContain("Sonnet 5.5");
  });
});

describe("MainWindow: the review's critical findings", () => {
  const BIG = { width: 1700, height: 900 } as const;
  it("zoom makes the zoomed pane the focused one: header, accent bar, its prompt line and the status bar all agree", () => {
    const h = html({ layout: "split", splitCount: 4, zoomedPane: 1, activePane: 0, ...BIG });
    expect(h).toContain('aria-label="Session 1, terminal: billing"');
    expect(h).toMatch(/<section class="pane" data-active="true"[^>]*aria-label="Session 1, terminal: billing"/);
    expect(h).toContain('aria-label="Write to billing"');
    expect(h).not.toContain('aria-label="Write to api-server"');
  });
  it("a chosen arrangement that no longer suits the number of panes falls back to automatic instead of crashing", () => {
    expect(() => html({ layout: "split", splitCount: 3, arrangement: "main-left", width: 1180, height: 800 })).not.toThrow();
    const h = html({ layout: "split", splitCount: 3, arrangement: "main-left", width: 1180, height: 800, layoutMenu: true });
    expect(h).toContain('data-auto="true"');
    expect(h).toContain("Showing 2 of 3");
  });
  it("copes with an arrangement picked for four after the window shrinks to one", () => {
    expect(() => html({ layout: "split", splitCount: 4, arrangement: "grid", width: 720, height: 480 })).not.toThrow();
  });
  it("copes with a focused pane index that is out of range: it clamps to the first or the last pane", () => {
    for (const [activePane, name] of [[-1, "api-server"], [9, "billing"]] as const) {
      const h = html({ layout: "split", splitCount: 2, activePane, ...BIG });
      expect((h.match(/data-active="true"/g) ?? []).length, String(activePane)).toBe(1);
      expect(h, String(activePane)).toMatch(new RegExp(`<section class="pane" data-active="true"[^>]*aria-label="Session ${NUM[name]}, terminal: ${name}"`));
    }
  });
  it("tells the layout menu how many sessions could be shown, not how many were asked for", () => {
    const two = SCENARIO_ROWS.slice(0, 2);
    const h = html({ layout: "split", splitCount: 4, rows: two, layoutMenu: true, ...BIG });
    expect(h).not.toContain("Showing 2 of 4");
  });
});

describe("MainWindow: the sidebar footer", () => {
  it("defaults to the scenario's plan and doctor numbers, so the stories do not change", () => {
    const h = html();
    expect(h).toContain("5h 38%");
    expect(h).toContain("Doctor: 4 issues");
  });
  it("shows no footer, and no made-up plan or doctor numbers, when given null", () => {
    const h = html({ footer: null });
    expect(h).not.toContain("sidebar__footer");
    expect(h).not.toContain("Doctor:");
    expect(h).not.toContain("5h ");
  });
  it("shows the numbers it is given", () => {
    expect(html({ footer: { doctorIssues: 2 } })).toContain("Doctor: 2 issues");
  });
});

describe("MainWindow: the shell panel in the sidebar", () => {
  const shellOf = (h: string) => h.slice(h.indexOf('<section class="shell-panel"'), h.indexOf("</section>", h.indexOf('<section class="shell-panel"')));
  const tall = { width: 1180, height: 880 } as const;
  it("is in the expanded sidebar by default, under the session list and above the footer", () => {
    const h = html(tall);
    expect(h).toContain('aria-label="Shell"');
    expect(h.indexOf('role="listbox"')).toBeLessThan(h.indexOf('aria-label="Shell"'));
    expect(h.indexOf('aria-label="Shell"')).toBeLessThan(h.indexOf("sidebar__footer"));
  });
  it("is plain PowerShell on Windows, in the focused session's folder", () => {
    const sh = shellOf(html(tall));
    expect(sh).toContain("PowerShell");
    expect(sh).toContain(">my-app<");
  });
  it("is not an agent: its panel carries no vendor, so the window's vendor accent does not make it look like a session", () => {
    expect(shellOf(html(tall))).not.toContain("data-agent");
  });
  it("follows the focused pane in split view", () => {
    const h = html({ layout: "split", splitCount: 2, activePane: 1, width: 1700, height: 900 });
    expect(shellOf(h)).toContain(">payments<");
  });
  it("is zsh on macOS and bash on Linux", () => {
    expect(shellOf(html({ ...tall, os: "mac" }))).toContain("zsh");
    expect(shellOf(html({ ...tall, os: "linux" }))).toContain("bash");
  });
  it("is not in the rail or in focus mode, where there is no room for it", () => {
    expect(html({ layout: "rail" })).not.toContain('aria-label="Shell"');
    expect(html({ layout: "focus", sidebarPref: "focus" })).not.toContain('aria-label="Shell"');
    expect(html({ width: 940 })).not.toContain('aria-label="Shell"');
  });
  it("can be left out", () => {
    expect(html({ ...tall, shell: false })).not.toContain('aria-label="Shell"');
  });
  it("shows only its header when the user hid it", () => {
    const sh = shellOf(html({ ...tall, shellCollapsed: true }));
    expect(sh).toContain('data-collapsed="true"');
    expect(sh).not.toContain("scripted-terminal");
    expect(sh).toContain('aria-label="Shell panel"');
  });
  it("hides itself in a window too short to leave the list two rows, when the lane takes its 40%", () => {
    const many = Array.from({ length: 12 }, (_, i) => ({ ...SCENARIO_REQUESTS[0]!, id: `r${i}`, waitingMs: 1000 + i }));
    const sh = shellOf(html({ width: 1180, height: 520, requests: many }));
    expect(sh).toContain('data-collapsed="true"');
    expect(sh).toContain('data-auto="true"');
  });
  it("a hidden shell panel stays folded even when there is room", () => {
    const sh = shellOf(html({ ...tall, shell: { hidden: true } }));
    expect(sh).toContain('data-collapsed="true"');
    // The user's choice, not the list running out of room.
    expect(sh).not.toContain("data-auto");
    expect(shellOf(html({ ...tall, shell: { hidden: false } }))).not.toContain("data-collapsed");
  });
  it("opens an auto-folded panel when the user asked for it", () => {
    const many = Array.from({ length: 12 }, (_, i) => ({ ...SCENARIO_REQUESTS[0]!, id: `r${i}`, waitingMs: 1000 + i }));
    const short = { width: 1180, height: 520, requests: many } as const;
    expect(shellOf(html(short))).toContain('data-auto="true"');
    const sh = shellOf(html({ ...short, shell: { forceOpen: true } }));
    expect(sh).not.toContain("data-collapsed");
    expect(shellOf(html({ ...short, shell: { forceOpen: true, hidden: true } }))).toContain('data-collapsed="true"');
  });
  it("shows the live terminal it is given, in the folder it is given", () => {
    const sh = shellOf(html({ ...tall, shell: { terminal: <div data-testid="live" />, cwd: "elsewhere" } }));
    expect(sh).toContain('data-testid="live"');
    expect(sh).not.toContain("scripted-terminal");
    expect(sh).toContain(">elsewhere<");
  });
  it("no shell panel in the rail or focus mode", () => {
    const live = { terminal: <div data-testid="live" /> };
    for (const p of [{ layout: "rail" as const }, { layout: "focus" as const, sidebarPref: "focus" as const }, { sidebarPref: "rail" as const }, { width: 940 }]) {
      const h = html({ ...tall, ...p, shell: live });
      expect(h).not.toContain('aria-label="Shell"');
      expect(h).not.toContain('data-testid="live"');
    }
  });
  it("has its own button for opening it in a pane, named for the shell", () => {
    expect(html(tall)).toContain('aria-label="Open PowerShell in a pane"');
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

describe("MainWindow: with no sessions", () => {
  const variants = {
    single: {},
    split: { layout: "split" as const, splitCount: 3 as const },
    rail: { layout: "rail" as const },
    focus: { layout: "focus" as const, sidebarPref: "focus" as const },
    "split in a small window": { layout: "split" as const, splitCount: 4 as const, width: 720, height: 480 },
  };
  for (const [name, p] of Object.entries(variants)) {
    it(`renders the empty state instead of throwing (${name})`, () => {
      const h = html({ rows: [], requests: [], selectedId: "none", ...p });
      expect(h).toContain("No sessions yet. Start one with Ctrl+Shift+T.");
      expect(h).not.toContain('aria-label="Terminal for');
      expect(h).not.toContain('<form class="composer"');
    });
  }
  it("says the launcher shortcut for the platform", () => {
    expect(html({ rows: [], requests: [], selectedId: "none", os: "mac" })).toContain("No sessions yet. Start one with ⌘T.");
  });
  it("still has the bar, the layout button and the window controls", () => {
    const h = html({ rows: [], requests: [], selectedId: "none" });
    for (const s of ["Marshell menu", "Split layout", "Minimize", "Command palette"]) expect(h, s).toContain(`aria-label="${s}"`);
    expect(h).not.toContain("session-header");
  });
});

describe("MainWindow: the layout menu while a pane is zoomed", () => {
  it("does not say 'Showing 1 of N' as if the window were too small: the pane header already says the others keep running", () => {
    const h = html({ layout: "split", splitCount: 3, width: 1700, height: 900, zoomedPane: 0, layoutMenu: true });
    expect(h).toContain("more running");
    expect(h).not.toMatch(/Showing \d+ of \d+/);
  });
  it("still says it when the window really is too small for the count asked for", () => {
    expect(html({ layout: "split", splitCount: 4, width: 720, height: 480, layoutMenu: true })).toMatch(/Showing 1 of 4/);
  });
});

describe("MainWindow: the brand button in focus mode", () => {
  it("is not rendered when the bar's side column is hidden, so keyboard focus can never land on something nobody sees", () => {
    const h = html({ layout: "focus", sidebarPref: "focus" });
    expect(h).not.toContain("window__brand");
    expect(h).not.toContain("Marshell menu");
    expect(h).not.toContain("Command palette");
  });
  it("is there in the expanded sidebar and in the rail", () => {
    expect(html()).toContain("Marshell menu");
    expect(html({ layout: "rail" })).toContain("Marshell menu");
  });
});

describe("MainWindow: renderTerminal", () => {
  const live = (row: { id: string }) => <div data-live={row.id} />;
  it("renderTerminal replaces the scripted terminal", () => {
    const h = html({ renderTerminal: live });
    expect(h).toContain('data-live="working"');
    expect(h).not.toContain("aria-label=\"Terminal for");
  });
  it("keeps the single window's terminal in a wrapper that can be hidden, and shows the selected one", () => {
    const h = html({ renderTerminal: live });
    expect(h).toMatch(/<div class="window__terminal"><div data-live="working"/);
    expect(h).not.toMatch(/<div[^>]* hidden[ =>][^>]*><div data-live/);
  });
  it("replaces the scripted terminal in every pane of the split view", () => {
    const h = html({ layout: "split", splitCount: 2, ...BIG, renderTerminal: live });
    expect((h.match(/data-live="/g) ?? []).length).toBe(2);
    expect(h).not.toContain("aria-label=\"Terminal for");
  });
  it("shows the ports it is given instead of the scenario's", () => {
    expect(html()).toContain("localhost:3000");
    expect(html({ ports: [] })).not.toContain("localhost:3000");
  });
});

describe("MainWindow: one live region, and the window's title follows the focused session", () => {
  const live = (h: string) => (h.match(/data-session-announcer/g) ?? []).length;
  it("has exactly one session announcer, whether it shows one terminal or four, and none of its prompt lines or panes is a live region", () => {
    for (const p of [{}, { layout: "split" as const, splitCount: 2 as const, ...BIG }, { layout: "split" as const, splitCount: 4 as const, ...BIG }]) {
      const h = html(p);
      expect(live(h)).toBe(1);
      const body = h.slice(h.indexOf('class="window__body"'));
      for (const sec of body.match(/<section class="pane[\s\S]*?<\/section>/g) ?? []) expect(sec).not.toMatch(/role="status"|aria-live=/);
      expect(body.match(/<form class="composer"[\s\S]*?<\/form>/g)?.join("") ?? "").not.toMatch(/role="status"|aria-live=/);
    }
  });
  it("renders the announcer empty on first render, so nothing is read out as the window opens", () => {
    const h = html({ selectedId: "needs-permission" });
    expect(/<div[^>]*data-session-announcer[^>]*>([^<]*)<\/div>/.exec(h)?.[1]).toBe("");
  });
  it("is a polite status region that is hidden visually", () => {
    expect(html()).toMatch(/<div class="window__announcer" role="status" aria-live="polite" aria-atomic="true" data-session-announcer/);
  });
});

describe("MainWindow: panes from the view state", () => {
  const ids = SCENARIO_ROWS.map((r) => r.id);
  const panes = (h: string) => (h.match(/<section class="pane"/g) ?? []).length;
  it("paneIds beyond the fit are hidden, not dropped", () => {
    const h = html({ layout: "split", paneIds: ids.slice(0, 4), layoutMenu: true, width: 760, height: 520 });
    expect(panes(h)).toBeLessThan(4);
    expect(h).toContain("Showing");
    const focused = h.split("</header>").find((part) => part.includes('data-active="true"'))!;
    expect(focused).toContain("more running");
  });
  it("shows the panes in paneIds order, not selected-first", () => {
    const [a, b] = [SCENARIO_ROWS[0]!, SCENARIO_ROWS[1]!];
    const h = html({ layout: "split", paneIds: [b.id, a.id], selectedId: a.id, ...BIG });
    const labels = [...h.matchAll(/<section class="pane"[^>]*aria-label="([^"]+)"/g)].map((m) => m[1]);
    expect(labels[0]).toContain(`terminal: ${b.name}`);
    expect(labels[1]).toContain(`terminal: ${a.name}`);
  });
  it("paneIds null renders the empty pane with the other sessions", () => {
    const h = html({ layout: "split", paneIds: [ids[0]!, null], ...BIG });
    expect(panes(h)).toBe(1);
    expect(h).toContain("pane--empty");
    const choices = h.slice(h.indexOf("pane--empty"));
    expect(choices).toContain("pane__choices");
    for (const r of SCENARIO_ROWS.slice(1, 3)) expect(choices).toContain(r.name);
  });
  it("a pane id that is no longer a session is an empty pane, not a crash", () => {
    const h = html({ layout: "split", paneIds: [ids[0]!, "gone"], ...BIG });
    expect(h).toContain("pane--empty");
  });
  it("window title follows the focused pane", () => {
    const [a, b] = [SCENARIO_ROWS[0]!, SCENARIO_ROWS[1]!];
    const title = (activePane: number) => html({ layout: "split", paneIds: [a.id, b.id], activePane, ...BIG }).match(/class="window"[^>]*data-title="([^"]*)"/)?.[1];
    expect(title(0)).toBe(windowTitle(a));
    expect(title(1)).toBe(windowTitle(b));
  });
  it("drives the arrangement, ratios and zoom from its props", () => {
    const h = html({ layout: "split", paneIds: ids.slice(0, 2), arrangement: "rows", ratios: { row: 0.3 }, ...BIG });
    expect(h).toContain('data-arrangement="rows"');
    expect(h).toContain("0.3fr");
    const z = html({ layout: "split", paneIds: ids.slice(0, 3), zoomedPane: 1, ...BIG });
    expect(panes(z)).toBe(1);
    expect(z).toContain('data-arrangement="single"');
  });
  it("makes the main area a focus target for the no-terminal shortcut", () => {
    expect(html({ layout: "split", paneIds: ids.slice(0, 2), ...BIG })).toMatch(/<main class="window__main"[^>]*tabindex="-1"/);
  });
});

describe("MainWindow: focus mode and the sidebar width", () => {
  it("focus mode renders no sidebar, no rail and no prompt line", () => {
    const h = html({ width: 1280, height: 800, sidebarPref: "focus" });
    expect(h).toContain('data-mode="focus"');
    expect(h).not.toContain('class="sidebar"');
    expect(h).not.toContain('class="rail"');
    expect(h).not.toContain("window__composer");
    expect(h).not.toContain('role="separator"');
    expect(h).toContain("--side-w:0px");
  });
  it("keeps the pane headers in focus mode", () => {
    const h = html({ width: 1280, height: 800, sidebarPref: "focus", layout: "split" });
    expect(h).toContain("pane__header");
  });
  it("offers an 8 px strip at the top edge to peek at the header, in focus mode only", () => {
    expect(html({ sidebarPref: "focus" })).toContain("window__peek");
    expect(html({ sidebarPref: "expanded" })).not.toContain("window__peek");
    expect(html({ sidebarPref: "focus", headerPeek: true })).not.toContain("window__peek");
  });
  it("the sidebar handle reports its width", () => {
    const h = html({ width: 1280, sidebarWidth: 320 });
    expect(h).toContain("--side-w:320px");
    expect(h).toMatch(/<div class="window__side-handle"[^>]*role="separator"[^>]*aria-orientation="vertical"[^>]*aria-valuenow="320"/);
    expect(h).toContain('aria-valuemin="200"');
    expect(h).toContain('aria-valuemax="400"');
    expect(h).toContain('aria-label="Resize the sidebar"');
  });
  it("holds a stored width to 200 to 400", () => {
    expect(html({ width: 1280, sidebarWidth: 120 })).toContain("--side-w:200px");
    expect(html({ width: 1280, sidebarWidth: 900 })).toContain("--side-w:400px");
  });
  it("below 960 the stored width does not apply: the rail is 52", () => {
    const h = html({ width: 900, sidebarWidth: 360 });
    expect(h).toContain("--side-w:52px");
    expect(h).toContain('class="rail"');
    expect(h).not.toContain("window__side-handle");
  });
  it("has no handle on the rail", () => {
    expect(html({ width: 1280, sidebarPref: "rail" })).not.toContain("window__side-handle");
  });
  it("makes the handle a tab stop only when it can be used", () => {
    expect(html({ sidebarWidth: 320 })).not.toMatch(/window__side-handle[^>]*tabindex/);
    expect(html({ sidebarWidth: 320, onSidebarWidth: () => {} })).toMatch(/window__side-handle[^>]*tabindex="0"/);
  });
});

describe("MainWindow: keyboard navigation", () => {
  it("shows that nothing is selected: no pane is active, and the status bar says so", () => {
    const h = html({ layout: "split", noneSelected: true });
    expect(h).toContain("No session selected · Tab to select a terminal");
    expect(h).not.toContain('data-active="true"');
  });
  it("says it without a status bar too (the live window has none)", () => {
    expect(html({ layout: "split", noneSelected: true, statusBar: false })).toContain("No session selected · Tab to select a terminal");
  });
  it("draws the cursor and its hint inside the window", () => {
    const h = html({ navCursor: { rect: { x: 0, y: 40, w: 288, h: 56 }, label: "Session 1, billing" } });
    expect(h).toContain('class="nav-cursor"');
    expect(h).toContain("Release Ctrl+Shift to select · Backspace for none");
  });
  it("marks what the cursor can reach: every pane, every session row and the top bar's controls", () => {
    const h = html({ layout: "split" });
    expect(h).toContain('data-nav-pane="0"');
    expect(h).toContain('data-nav-pane="1"');
    expect(h).toMatch(/data-nav-row="\d+"/);
    expect(h).toContain('data-nav-top="palette"');
    expect(h).toContain('data-nav-top="layout"');
    expect(html({ width: 940 })).toMatch(/<button[^>]*rail__item[^>]*data-nav-row="\d+"|<button[^>]*data-nav-row="\d+"[^>]*rail__item/);
    expect(html()).toContain('data-nav-pane="0"');
  });
});

describe("MainWindow: the restart bar", () => {
  const bar = <div className="close-bar">Restart 1 billing in Plan?</div>;
  it("with two panes, replaces the target pane's header and the top bar holds no bar", () => {
    const h = html({ layout: "split", splitCount: 2, ...BIG, paneBar: { id: "needs-permission", node: bar } });
    const at = h.indexOf('class="split"');
    expect(h.slice(0, at)).not.toContain("close-bar");
    expect((h.match(/close-bar/g) ?? []).length).toBe(1);
    const billing = h.slice(at).split("<section").find((s) => s.includes("terminal: billing"));
    expect(billing).toContain("close-bar");
    expect(billing).not.toContain("<header");
    expect((h.match(/<header class="pane__header"/g) ?? []).length).toBe(1);
  });
  it("with one pane, goes in the top bar", () => {
    const h = html({ paneBar: { id: "working", node: bar } });
    const from = h.indexOf("window__bar-main");
    const top = h.slice(from, h.indexOf("</header>", from));
    expect(top).toContain("close-bar");
    expect(top).not.toContain("session-header");
  });
  it("a bar for a session on no pane on screen goes in the top bar too", () => {
    const h = html({ layout: "split", splitCount: 2, ...BIG, paneBar: { id: "not-shown", node: bar } });
    expect((h.match(/close-bar/g) ?? []).length).toBe(1);
  });
});

describe("MainWindow: the sidebar toggle", () => {
  const toggle = (h: string) => h.match(/<button[^>]*class="window__sidebar-toggle"[^>]*>/)?.[0];
  it("follows the palette button in the expanded bar, with aria-expanded true", () => {
    const h = html({ width: 1280 });
    expect(h.indexOf("window__sidebar-toggle")).toBeGreaterThan(h.indexOf("window__palette"));
    const t = toggle(h)!;
    expect(t).toContain('data-nav-top="sidebar"');
    expect(t).toContain('aria-label="Sidebar"');
    expect(t).toContain('aria-expanded="true"');
    expect(t).toContain('title="Collapse sidebar"');
    expect(t).not.toContain("aria-disabled");
  });
  it("stays in the rail's bar with aria-expanded false, beside an icon-only palette button", () => {
    const h = html({ width: 1280, sidebarPref: "rail" });
    const t = toggle(h)!;
    expect(t).toContain('aria-expanded="false"');
    expect(t).toContain('title="Expand sidebar"');
    expect(h).toMatch(/class="window__palette"[^>]*aria-label="Command palette"/);
    expect(h).not.toContain("<span>Command palette</span>");
  });
  it("has no buttons in the bar's side column in focus mode", () => {
    const h = html({ sidebarPref: "focus" });
    expect(h).not.toContain("window__sidebar-toggle");
    expect(h).not.toContain("window__palette");
  });
  it("is aria-disabled, still focusable, when a narrow window keeps the sidebar from opening", () => {
    const t = toggle(html({ width: 900, sidebarPref: "expanded" }))!;
    expect(t).toContain('aria-disabled="true"');
    expect(t).not.toMatch(/\sdisabled/);
    expect(t).toContain('title="Widen the window to open the sidebar"');
  });
  it("is not disabled in the rail the user chose, even in a narrow window", () => {
    expect(toggle(html({ width: 900, sidebarPref: "rail" }))).not.toContain("aria-disabled");
  });
  it("the rail no longer renders a palette button", () => {
    expect(html({ width: 1280, sidebarPref: "rail" })).not.toContain("rail__tool");
  });
});
