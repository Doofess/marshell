import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MainWindow } from "./MainWindow";
import { COMPOSER, STATUS_BAR_HEIGHT } from "./layout";
import { SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { windowTitle } from "./windowTitle";
import type { SessionInfo } from "../../generated/SessionInfo";
import { offersModeChange } from "../sessions/resumeState";
import { topAction } from "../sessions/useNavigation";

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
  it("keeps the window's own controls (the panel, the cursor) in the app accent: only a session's own elements take its vendor's", () => {
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
  it("shows the text typed in the window", () => {
    expect(html({ composerValue: "Add retry tests" })).toContain("Add retry tests");
  });
});

describe("MainWindow: the status bar follows the selected session", () => {
  const BIG = { width: 1700, height: 900 } as const;
  const bar = (h: string) => /<div class="window__status"[^>]*>([\s\S]*?)<\/div><\/div>/.exec(h)?.[0] ?? "";
  it("shows the selected session: its model", () => {
    expect(bar(html({ selectedId: "working" }))).toContain("Opus 5.5");
    expect(bar(html({ selectedId: "needs-permission" }))).toContain("Sonnet 5.5");
  });
  it("is a 24 px bar at the foot of the centre column, with the window's own height from the layout constants", () => {
    const h = html();
    expect(h).toMatch(/class="window__status"[^>]*style="block-size:\d+px"/);
    expect(h).toContain(`block-size:${STATUS_BAR_HEIGHT}px`);
  });
  it("shows in the window, and is gone in focus mode or when turned off", () => {
    expect(html()).toContain("window__status");
    expect(html({ layout: "focus", sidebarPref: "focus" })).not.toContain("window__status");
    expect(html({ statusBar: false })).not.toContain("window__status");
  });
});

describe("MainWindow: the sidebar footer", () => {
  it("defaults to the scenario's plan and doctor numbers, so the stories do not change", () => {
    const h = html();
    expect(h).toContain("5h 38%");
    expect(h).toContain("Doctor: 3 issues");
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
  it("follows the selected session", () => {
    const h = html({ selectedId: "needs-permission", width: 1700, height: 900 });
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
  it("has its own button for opening it as a session, named for the shell", () => {
    expect(html(tall)).toContain('aria-label="Open PowerShell as a session"');
  });
});

describe("MainWindow: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    for (const layout of ["default", "focus", "rail"] as const)
      expect(html({ layout, sidebarPref: layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded" })).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});

describe("MainWindow (accessibility gate)", () => {
  it("gives the focusable preview separator the values a separator needs", () => {
    const h = html({ pane: <aside className="preview-stand-in" />, paneShown: true });
    expect(h).toContain('role="separator"');
    expect(h).toMatch(/aria-valuenow="\d+"/);
    expect(h).toContain('aria-valuemin="320"');
    expect(h).toMatch(/aria-valuemax="\d+"/);
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
    rail: { layout: "rail" as const },
    focus: { layout: "focus" as const, sidebarPref: "focus" as const },
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
  it("still has the bar and the window controls", () => {
    const h = html({ rows: [], requests: [], selectedId: "none" });
    for (const s of ["Marshell menu", "Minimize", "Command palette"]) expect(h, s).toContain(`aria-label="${s}"`);
    expect(h).not.toContain("session-header");
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
  it("shows the ports it is given, and none of its own", () => {
    expect(html()).not.toContain("localhost:3000");
    expect(html({ ports: ["localhost:3000"] })).toContain("localhost:3000");
    expect(html({ ports: [] })).not.toContain("localhost:3000");
  });
});

describe("MainWindow: one live region, and the window's title follows the focused session", () => {
  const live = (h: string) => (h.match(/data-session-announcer/g) ?? []).length;
  it("has exactly one session announcer, and none of its prompt lines is a live region", () => {
    const h = html();
    expect(live(h)).toBe(1);
    const body = h.slice(h.indexOf('class="window__body"'));
    expect(body.match(/<form class="composer"[\s\S]*?<\/form>/g)?.join("") ?? "").not.toMatch(/role="status"|aria-live=/);
  });
  it("renders the announcer empty on first render, so nothing is read out as the window opens", () => {
    const h = html({ selectedId: "needs-permission" });
    expect(/<div[^>]*data-session-announcer[^>]*>([^<]*)<\/div>/.exec(h)?.[1]).toBe("");
  });
  it("is a polite status region that is hidden visually", () => {
    expect(html()).toMatch(/<div class="window__announcer" role="status" aria-live="polite" aria-atomic="true" data-session-announcer/);
  });
});

describe("MainWindow: the window's title", () => {
  const [a, b] = [SCENARIO_ROWS[0]!, SCENARIO_ROWS[1]!];
  const title = (selectedId: string, needsYou = 0) => html({ selectedId, needsYou }).match(/class="window"[^>]*data-title="([^"]*)"/)?.[1];
  it("names the selected session", () => {
    expect(title(a.id)).toBe(windowTitle(a, 0));
    expect(title(b.id)).toBe(windowTitle(b, 0));
  });
  it("leads with the needs-you count it is given", () => {
    expect(title(a.id, 2)).toBe(windowTitle(a, 2));
    expect(title(a.id, 2)).toMatch(/^\(2\) /);
  });
  it("makes the main area a focus target for the no-terminal shortcut", () => {
    expect(html()).toMatch(/<main class="window__main"[^>]*tabindex="-1"/);
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
  it("shows that nothing is selected, and the status bar says so", () => {
    expect(html({ noneSelected: true })).toContain("No session selected · Tab to select a terminal");
  });
  it("says it without a status bar too (the live window has none)", () => {
    expect(html({ noneSelected: true, statusBar: false })).toContain("No session selected · Tab to select a terminal");
  });
  it("draws the cursor and its hint inside the window", () => {
    const h = html({ navCursor: { rect: { x: 0, y: 40, w: 288, h: 56 }, label: "Session 1, billing" } });
    expect(h).toContain('class="nav-cursor"');
    expect(h).toContain("Release Ctrl+Shift to select · Backspace for none");
  });
  it("marks what the cursor can reach: the terminal, every session row and the top bar's controls", () => {
    const h = html();
    expect(h).toContain('data-nav-pane="0"');
    expect(h).not.toContain('data-nav-pane="1"');
    expect(h).toMatch(/data-nav-row="\d+"/);
    expect(h).toContain('data-nav-top="palette"');
    expect(html({ width: 940 })).toMatch(/<button[^>]*rail__item[^>]*data-nav-row="\d+"|<button[^>]*data-nav-row="\d+"[^>]*rail__item/);
  });
});

describe("MainWindow: the restart bar", () => {
  const bar = <div className="close-bar">Restart 1 billing in Plan?</div>;
  it("goes in the top bar in place of the session header", () => {
    const h = html({ paneBar: { id: "working", node: bar } });
    const from = h.indexOf("window__bar-main");
    const top = h.slice(from, h.indexOf("</header>", from));
    expect(top).toContain("close-bar");
    expect(top).not.toContain("session-header");
  });
  it("a bar for a session that is not on screen goes in the top bar too", () => {
    const h = html({ paneBar: { id: "not-shown", node: bar } });
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

describe("MainWindow: the side pane toggle", () => {
  const stand = <aside className="preview-stand-in" />;
  const toggle = (h: string) => h.match(/<button[^>]*class="window__pane-toggle"[^>]*>/)?.[0];
  it("is the first child of the bar's end side, before the window controls", () => {
    const h = html();
    const end = h.indexOf('class="window__bar-end"');
    expect(end).toBeGreaterThan(-1);
    expect(h.indexOf("window__pane-toggle")).toBeGreaterThan(end);
    expect(h.indexOf("window__pane-toggle")).toBeLessThan(h.indexOf("window__controls"));
    expect(h.slice(end, end + 200)).toMatch(/^[^>]*data-tauri-drag-region[^>]*><button[^>]*class="window__pane-toggle"/);
  });
  it("says Show with aria-expanded false while the pane is hidden", () => {
    const t = toggle(html())!;
    expect(t).toContain('data-nav-top="pane"');
    expect(t).toContain('aria-label="Side pane"');
    expect(t).toContain('aria-expanded="false"');
    expect(t).toContain('title="Show the side pane (Ctrl+Shift+Q)"');
    expect(t).not.toContain("aria-disabled");
    expect(html()).toContain('data-icon="side-pane"');
  });
  it("says Hide with aria-expanded true while the pane is shown", () => {
    const t = toggle(html({ pane: stand, paneShown: true }))!;
    expect(t).toContain('aria-expanded="true"');
    expect(t).toContain('title="Hide the side pane (Ctrl+Shift+Q)"');
  });
  it("stays in the bar in the rail, in focus mode and on another OS", () => {
    expect(toggle(html({ sidebarPref: "rail" }))).toBeDefined();
    expect(toggle(html({ sidebarPref: "focus" }))).toBeDefined();
    const mac = html({ os: "mac" });
    expect(toggle(mac)).toBeDefined();
    expect(mac).not.toContain("window__controls");
  });
  it("is aria-disabled, not disabled, with no session selected", () => {
    const t = toggle(html({ rows: [], selectedId: undefined, pane: stand, paneShown: true }))!;
    expect(t).toContain('aria-disabled="true"');
    expect(t).not.toMatch(/\sdisabled/);
    expect(t).toContain('aria-expanded="false"');
    expect(t).toContain('title="The side pane shows beside a session"');
  });
  it("is aria-disabled while a main view replaces the terminal", () => {
    const t = toggle(html({ mainView: <section />, pane: stand, paneShown: true }))!;
    expect(t).toContain('aria-disabled="true"');
    expect(t).toContain('aria-expanded="false"');
    expect(t).toContain('title="The side pane shows beside a session"');
  });
});

describe("MainWindow: one terminal", () => {
  it("the top bar has no layout button", () => {
    const h = html({ rows: SCENARIO_ROWS.slice(0, 3) });
    expect(h).not.toContain("Split layout");
    expect(h).not.toContain('data-nav-top="layout"');
    expect(h).not.toContain('class="split');
  });
  it("every top-bar control the cursor reaches does something when the cursor is released on it", () => {
    const ids = new Set<string>();
    for (const p of [{}, { sidebarPref: "rail" as const }, { width: 900 }]) {
      for (const m of html({ rows: SCENARIO_ROWS.slice(0, 3), ...p }).matchAll(/data-nav-top="([^"]+)"/g)) ids.add(m[1]!);
    }
    expect([...ids].sort()).toEqual(["palette", "pane", "sidebar"]);
    for (const id of ids) expect(topAction(id), id).not.toBeNull();
  });
  it("the divider reports the pane's width in pixels", () => {
    const stand = <aside className="preview-stand-in" />;
    const h = html({ pane: stand, paneShown: true, paneWidth: 400 });
    expect(h).toContain('class="pane__divider"');
    expect(h).toContain('role="separator"');
    expect(h).toContain('aria-label="Resize the side pane"');
    expect(h).toContain('aria-valuenow="400"');
    expect(h).toContain("px 1px 400px");
    expect(html({ pane: stand, paneShown: true })).toMatch(/aria-valuenow="\d+"/);
  });
  it("a cover pane has no divider", () => {
    const h = html({ pane: <aside className="preview-stand-in" />, paneShown: true, width: 700 });
    expect(h).toContain('data-pane="cover"');
    expect(h).toContain("preview-stand-in");
    expect(h).not.toContain("pane__divider");
  });
  it("a pane that is not shown leaves the terminal alone", () => {
    const h = html({ pane: <aside className="preview-stand-in" /> });
    expect(h).not.toContain("preview-stand-in");
    expect(h).not.toContain("data-pane");
  });
});

describe("MainWindow: mainView", () => {
  const view = <section className="history-stand-in">History</section>;
  const main = (h: string) => /<main[^>]*class="window__main".*?<\/main>/s.exec(h)![0];
  it("holds the view in the main area in place of the terminal, the empty text and the preview", () => {
    const h = html({ mainView: view, pane: <aside className="preview-stand-in" />, paneShown: true });
    expect(main(h)).toContain("history-stand-in");
    expect(main(h)).not.toContain("scripted-terminal");
    expect(main(h)).not.toContain("preview-stand-in");
    expect(html({ mainView: view, rows: [] })).toContain("history-stand-in");
    expect(html({ mainView: view, rows: [] })).not.toContain("No sessions yet");
  });
  it("keeps the sidebar and the session header", () => {
    const h = html({ mainView: view });
    expect(h).toContain('class="sidebar"');
    expect(h).toContain("session-header");
  });
  it("shows the terminal as before without it", () => {
    expect(html()).toContain("scripted-terminal");
    expect(html()).not.toContain("history-stand-in");
  });
});
