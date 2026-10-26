import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { SETTINGS_SECTIONS, type SettingsSection } from "./settingsSections";
import { SettingsPanels } from "./LiveSettings";
import { closeOrRefuse, closesSettings, focusSettingsHeading, mountedWith, paletteSection, settingsStart, visibleControls } from "./settingsRouting";

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

describe("settingsStart", () => {
  it("opens the section shown last in this run, else Appearance", () => {
    expect(settingsStart(null)).toBe("appearance");
    expect(settingsStart("history")).toBe("history");
    expect(settingsStart("advanced")).toBe("advanced");
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
