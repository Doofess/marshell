import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { SETTINGS_SECTIONS, type CompanyAgent, type SettingsSection } from "./settingsSections";
import { SettingsPanels } from "./LiveSettings";
import { holdFocus, type GuardDom } from "../../lib/focusGuard";
import { closeOrRefuse, closesSettings, focusSettingsHeading, freshSettingsStart, isTextEntry, mountedWith, paletteSection, settingsFocusFallback, settingsStart, settingsTabTarget, visibleControls } from "./settingsRouting";

describe("paletteSection", () => {
  it.each([
    ["profiles", "profiles"],
    ["appearance", "appearance"],
    ["agents", "agents"],
    ["diagnostics", "advanced"],
    ["notifications", "notifications"],
    ["history-settings", "history"],
    ["settings", "appearance"],
  ] satisfies [string, SettingsSection][])("opens %s at %s", (id, section) => {
    expect(paletteSection(id)).toBe(section);
  });
  it("maps anything else to null, and only to sections the page has", () => {
    for (const id of ["departures", "cheatsheet", "history", "session:a", "focus-session-1", "terminal", "advanced", ""]) expect(paletteSection(id), id).toBeNull();
    for (const id of ["profiles", "appearance", "agents", "diagnostics", "notifications", "history-settings", "settings"]) expect(SETTINGS_SECTIONS.map((s) => s.id)).toContain(paletteSection(id));
  });
});

describe("paletteSection and the company sections", () => {
  it("has no palette command for Accounts or the company sections", () => {
    for (const id of ["accounts", "claude", "codex", "antigravity"]) expect(paletteSection(id), id).toBeNull();
  });
});

describe("settingsStart", () => {
  it("opens the section shown last in this run, else Appearance", () => {
    const all = SETTINGS_SECTIONS.map((s) => s.id);
    expect(settingsStart(null, all)).toBe("appearance");
    expect(settingsStart("history", all)).toBe("history");
    expect(settingsStart("advanced", all)).toBe("advanced");
  });
  it("falls back to Appearance when the last section is no longer listed", () => {
    const without = SETTINGS_SECTIONS.map((s) => s.id).filter((id) => id !== "codex");
    expect(settingsStart("codex", without)).toBe("appearance");
    expect(settingsStart("codex", [...without, "codex"])).toBe("codex");
  });
});

describe("freshSettingsStart", () => {
  const known = new Set<CompanyAgent>(["claude", "codex"]);
  it("reads the CLIs again before reopening at a CLI's section, and opens Appearance when that CLI is gone", async () => {
    const read = vi.fn(async () => new Set<CompanyAgent>(["claude"]));
    expect(await freshSettingsStart("codex", known, read)).toEqual({ start: "appearance", found: new Set(["claude"]) });
    expect(read).toHaveBeenCalledOnce();
  });
  it("reopens at the CLI's section when it is still found", async () => {
    expect(await freshSettingsStart("codex", known, async () => new Set<CompanyAgent>(["codex"]))).toEqual({ start: "codex", found: new Set(["codex"]) });
  });
  it("keeps what it knew when the read fails", async () => {
    expect(await freshSettingsStart("codex", known, () => Promise.reject(new Error("down")))).toEqual({ start: "codex", found: null });
  });
  it("doesn't read for any other section", async () => {
    const read = vi.fn(async () => new Set<CompanyAgent>());
    expect(await freshSettingsStart("history", known, read)).toEqual({ start: "history", found: null });
    expect(await freshSettingsStart(null, known, read)).toEqual({ start: "appearance", found: null });
    expect(read).not.toHaveBeenCalled();
  });
});

describe("closesSettings", () => {
  it("is true for a jump to a session: Ctrl+Shift+1 to 9, the oldest waiting and a palette session", () => {
    for (let n = 1; n <= 9; n++) expect(closesSettings(`focus-session-${n}`), String(n)).toBe(true);
    expect(closesSettings("next-waiting")).toBe(true);
    expect(closesSettings("session:tab-a")).toBe(true);
  });
  it("is false for everything else", () => {
    for (const id of ["palette", "history", "launcher", "close", "focus-session-0", "focus-session-10", "focus-session-", "review-task", "settings", "session", ""]) expect(closesSettings(id), id).toBe(false);
  });
});

describe("mountedWith", () => {
  it("mounts a section the first time it is shown, in the order shown", () => {
    expect(mountedWith([], "terminal")).toEqual(["terminal"]);
    expect(mountedWith(["terminal"], "agents")).toEqual(["terminal", "agents"]);
  });
  it("keeps one that is already mounted, and returns the same list so a render sets no state", () => {
    const mounted: SettingsSection[] = ["terminal", "agents"];
    expect(mountedWith(mounted, "terminal")).toBe(mounted);
  });
});

describe("SettingsPanels", () => {
  const html = (section: SettingsSection, mounted: SettingsSection[]) =>
    renderToStaticMarkup(<SettingsPanels section={section} mounted={mounted} render={(id) => <p>{`body of ${id}`}</p>} />);

  it("shows the section it was opened at and mounts no other", () => {
    const h = html("history", ["history"]);
    expect(h).toContain("body of history");
    expect(h).not.toContain("body of appearance");
    expect(h).not.toMatch(/hidden/);
  });
  it("renders accounts and codex through render, the shown one visible and the other hidden", () => {
    const h = html("codex", ["accounts", "codex"]);
    expect(h).toMatch(/<div[^>]*hidden=""[^>]*><p>body of accounts<\/p><\/div>/);
    expect(h).toMatch(/<div[^>]*><p>body of codex<\/p><\/div>/);
  });
  it("keeps a section that was shown mounted but hidden after a switch, so an unsaved form survives", () => {
    const h = html("agents", ["profiles", "agents"]);
    expect(h).toMatch(/<div[^>]*hidden=""[^>]*><p>body of profiles<\/p><\/div>/);
    expect(h).toMatch(/<div[^>]*><p>body of agents<\/p><\/div>/);
    expect(h.match(/hidden=""/g)).toHaveLength(1);
  });
  it("hides by the hidden attribute alone: no class of its own can override it", () => {
    expect(html("agents", ["profiles", "agents"])).not.toMatch(/<div[^>]*class=[^>]*hidden/);
  });
});

describe("focusSettingsHeading", () => {
  it("moves focus to the shown section's h2", () => {
    const heading = { focus: vi.fn() };
    const query = vi.fn(() => heading);
    focusSettingsHeading({ querySelector: query });
    expect(query).toHaveBeenCalledWith(".settings-shell__heading");
    expect(heading.focus).toHaveBeenCalledTimes(1);
  });
  it("does nothing without the page or its heading", () => {
    expect(() => focusSettingsHeading(null)).not.toThrow();
    expect(() => focusSettingsHeading({ querySelector: () => null })).not.toThrow();
  });
});

describe("visibleControls", () => {
  it("leaves out the controls of a section hidden after a switch", () => {
    const control = (hidden: boolean) => ({ closest: (sel: string) => (hidden && sel === "[hidden]" ? {} : null) });
    const shown = control(false);
    expect(visibleControls([shown, control(true), control(false)])).toHaveLength(2);
    expect(visibleControls([control(true)])).toEqual([]);
    expect(visibleControls([shown])[0]).toBe(shown);
  });

  it("stops once on a radio group, at its checked radio, as the browser's own Tab does", () => {
    const radio = (name: string, checked: boolean) => ({ closest: () => null, type: "radio", name, checked, tabIndex: 0 });
    const button = { closest: () => null, type: "button", name: "", checked: false, tabIndex: 0 };
    const [dark, light, system] = [radio("theme", false), radio("theme", true), radio("theme", false)];
    const [small, large] = [radio("size", false), radio("size", false)];
    expect(visibleControls([button, dark, light, system, small, large])).toEqual([button, light, small]);
  });

  it("leaves out a control taken out of the tab order", () => {
    const out = { closest: () => null, tabIndex: -1 };
    expect(visibleControls([out])).toEqual([]);
  });
});

describe("closeOrRefuse", () => {
  it("closes, unless the shell profiles hold unsaved edits", () => {
    expect(closeOrRefuse(true)).toBe("close");
    expect(closeOrRefuse(false)).toBe("refuse");
  });
});

describe("LiveSettings.css", () => {
  const css = readFileSync(new URL("./LiveSettings.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const rule = (sel: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter((m) => m[1]!.split(",").map((s) => s.trim()).includes(sel)).map((m) => m[2]).join(";");
  it("covers the window below the title bar, as Departures does, above the sidebar's resize handle", () => {
    expect(rule(".live-settings")).toMatch(/position:\s*absolute/);
    expect(rule(".live-settings")).toMatch(/inset:\s*40px 0 0 0/);
    expect(rule(".live-settings")).toMatch(/z-index:\s*20/);
    expect(rule(".live-settings")).toMatch(/min-block-size:\s*0/);
  });
  it("gives the page a Canvas background in forced colours", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)\s*\{\s*\.live-settings\s*\{\s*background:\s*Canvas;?\s*\}\s*\}/);
  });
  it("scrolls nowhere itself: the page body scrolls", () => {
    expect(css).not.toMatch(/overflow(?:-[xy])?:\s*(?:auto|scroll|overlay)/);
  });
});

describe("settingsFocusFallback", () => {
  it("is the shown section's heading, where stray focus goes", () => {
    const heading = { focus: vi.fn() };
    const query = vi.fn(() => heading);
    expect(settingsFocusFallback({ querySelector: query })).toBe(heading);
    expect(query).toHaveBeenCalledWith(".settings-shell__heading");
  });
  it("is null without the page or its heading", () => {
    expect(settingsFocusFallback(null)).toBeNull();
    expect(settingsFocusFallback({ querySelector: () => null })).toBeNull();
  });
  it("pulls focus back to the heading when the control that held it has left the page (the font field on Enter)", () => {
    const heading = { focus: vi.fn(), isConnected: true };
    const field = { isConnected: true, focus: vi.fn() };
    const page = { contains: (n: unknown) => n === heading || n === field, isConnected: true };
    let active: unknown = null;
    const focusIn: ((t: unknown) => void)[] = [];
    const frames: (() => void)[] = [];
    const none = () => () => {};
    const dom: GuardDom = {
      active: () => active,
      inOtherLayer: () => false,
      onFocusIn: (h) => (focusIn.push(h), none()),
      onFocusOut: none,
      onWindowFocus: none,
      onPress: none,
      later: (f) => (frames.push(f), none()),
    };
    const stop = holdFocus(page, dom, () => settingsFocusFallback({ querySelector: () => heading }), []);
    focusIn.forEach((h) => h(field));
    field.isConnected = false;
    active = null;
    focusIn.forEach((h) => h(null));
    frames.forEach((f) => f());
    expect(heading.focus).toHaveBeenCalledTimes(1);
    stop();
  });
});

describe("settingsTabTarget", () => {
  /** A control at a place in the document: compareDocumentPosition says whether another is before (2) or after (4) it. */
  const at = (place: number) => ({ place, compareDocumentPosition: (o: { place: number }) => (o.place > place ? 4 : o.place < place ? 2 : 0) });
  const [back, nav1, nav2, first, second, last] = [at(10), at(20), at(30), at(50), at(60), at(70)];
  const controls = [back, nav1, nav2, first, second, last];
  const heading = at(40);

  it("goes to the next control, and wraps, when the focus is on one of them", () => {
    expect(settingsTabTarget(controls, first, false)).toBe(4);
    expect(settingsTabTarget(controls, last, false)).toBe(0);
    expect(settingsTabTarget(controls, first, true)).toBe(2);
    expect(settingsTabTarget(controls, back, true)).toBe(5);
  });
  it("goes from the heading to the section's first control with Tab, and to the nav's last with Shift+Tab", () => {
    expect(settingsTabTarget(controls, heading, false)).toBe(3);
    expect(settingsTabTarget(controls, heading, true)).toBe(2);
  });
  it("wraps when no control follows (Tab) or precedes (Shift+Tab) the focus", () => {
    const after = at(99);
    const before = at(1);
    expect(settingsTabTarget(controls, after, false)).toBe(0);
    expect(settingsTabTarget(controls, after, true)).toBe(5);
    expect(settingsTabTarget(controls, before, true)).toBe(5);
    expect(settingsTabTarget(controls, before, false)).toBe(0);
  });
  it("starts at the first control (Tab) or the last (Shift+Tab) when nothing has the focus, and stays put without controls", () => {
    expect(settingsTabTarget(controls, null, false)).toBe(0);
    expect(settingsTabTarget(controls, null, true)).toBe(5);
    expect(settingsTabTarget([], heading, false)).toBeNull();
  });
});

describe("isTextEntry", () => {
  const input = (type: string | undefined) => ({ tagName: "INPUT", type });
  it("is true where typing a digit or letter belongs to the field", () => {
    for (const type of ["text", "search", "number", "email", "url", "tel", "password", "date", "time", "datetime-local", "month", "week"]) expect(isTextEntry(input(type)), type).toBe(true);
    expect(isTextEntry(input(undefined))).toBe(true);
    expect(isTextEntry(input("TEXT"))).toBe(true);
    expect(isTextEntry({ tagName: "TEXTAREA" })).toBe(true);
    expect(isTextEntry({ tagName: "DIV", isContentEditable: true })).toBe(true);
  });
  it("is false for a radio, a switch, a range, a color, a file and the buttons", () => {
    for (const type of ["radio", "checkbox", "range", "color", "file", "button", "submit", "reset", "image", "hidden"]) expect(isTextEntry(input(type)), type).toBe(false);
    for (const tagName of ["BUTTON", "SELECT", "A", "H2", "DIV"]) expect(isTextEntry({ tagName }), tagName).toBe(false);
    expect(isTextEntry(null)).toBe(false);
    expect(isTextEntry({})).toBe(false);
  });
  it("leaves the terminal's hidden textarea alone, as isTextBox does", () => {
    expect(isTextEntry({ tagName: "TEXTAREA", classList: { contains: (n: string) => n === "xterm-helper-textarea" } })).toBe(false);
  });
});
