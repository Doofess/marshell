import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ImportedTheme } from "../../generated/ImportedTheme";
import { addImported, removeImported } from "./appearanceEdits";
import { AppearanceView, type AppearanceHandlers, type AppearanceViewState } from "./AppearanceDialog";
import { DEFAULT_APPEARANCE } from "./rootState";

const noop = () => {};
const handlers: AppearanceHandlers = { save: noop, onAccentText: noop, commitAccent: noop, blurAccent: noop, pickAccent: noop, onFontText: noop, commitFont: noop, onSizeText: noop, commitSize: noop, importFile: noop };
const STATE: AppearanceViewState = { appearance: DEFAULT_APPEARANCE, app: "dark", wtFiles: [], error: null, accent: { text: "", reason: null }, font: "", size: { text: "13", reason: null }, importError: null };
const html = (over: Partial<AppearanceViewState> = {}) => renderToStaticMarkup(<AppearanceView state={{ ...STATE, ...over }} handlers={handlers} />);

const ANSI = Array.from({ length: 16 }, (_, i) => `#${(i * 16).toString(16).padStart(2, "0")}0000`);
const theme = (id: string, name = id): ImportedTheme => ({ id, name, source: "windows-terminal", colors: { foreground: "#ffffff", background: "#000000", cursor: "#ffffff", selection: "#333333", ansi: ANSI } });
const WT = { path: "C:/Users/me/AppData/Local/Packages/WT/LocalState/settings.json", text: JSON.stringify({ schemes: [{ name: "Campbell", foreground: "#cccccc", background: "#0c0c0c", black: "#0c0c0c", red: "#c50f1f", green: "#13a10e", yellow: "#c19c00", blue: "#0037da", purple: "#881798", cyan: "#3a96dd", white: "#cccccc", brightBlack: "#767676", brightRed: "#e74856", brightGreen: "#16c60c", brightYellow: "#f9f1a5", brightBlue: "#3b78ff", brightPurple: "#b4009e", brightCyan: "#61d6d6", brightWhite: "#f2f2f2" }] }) };

/** The radio input whose label says `name`. */
const radio = (markup: string, name: string) => markup.match(new RegExp(`<label[^>]*>(?:(?!</label>).)*?<input[^>]*>(?:(?!</label>).)*?>${name}<`, "s"))?.[0] ?? "";

describe("AppearanceView", () => {
  it("marks the current accent", () => {
    const m = html({ appearance: { ...DEFAULT_APPEARANCE, accent: "teal" } });
    expect(radio(m, "Teal")).toContain('checked=""');
    expect(radio(m, "Teal")).toContain('data-selected="true"');
    expect(radio(m, "Sand")).not.toContain('checked=""');
  });

  it("custom accent shows the reason and keeps the old value", () => {
    const m = html({ accent: { text: "#ffff00", reason: "Too light on a light background (1:1). Pick a darker colour." } });
    expect(m).toContain("Too light on a light background");
    expect(m).toMatch(/aria-describedby="[^"]+"[^>]*value="#ffff00"|value="#ffff00"[^>]*aria-describedby="[^"]+"/);
    expect(radio(m, "Sand")).toContain('checked=""');
  });

  it("lists Windows Terminal schemes with Add", () => {
    const m = html({ wtFiles: [WT] });
    expect(m).toContain("In your Windows Terminal settings");
    expect(m).toContain("Campbell");
    expect(m).toMatch(/aria-label="Add Campbell"/);
  });

  it("one bad Windows Terminal scheme is skipped and the others still list, with a note", () => {
    const bad = { path: "x", text: JSON.stringify({ schemes: [{ name: "Foo", foreground: "#ffffff" }, JSON.parse(WT.text).schemes[0]] }) };
    const m = html({ wtFiles: [bad] });
    expect(m).toMatch(/aria-label="Add Campbell"/);
    expect(m).toContain("Skipped 1 scheme: Foo is missing black.");
    expect(m).not.toContain("have no colour schemes");
    // When nothing in the files can be read the list says why instead of claiming there are no schemes.
    const none = html({ wtFiles: [{ path: "x", text: JSON.stringify({ schemes: [{ name: "Foo" }] }) }] });
    expect(none).toContain("Skipped 1 scheme: Foo is missing black.");
    expect(none).not.toContain("have no colour schemes");
  });

  it("empty Windows Terminal list says so", () => {
    expect(html({ wtFiles: [] })).toContain("No Windows Terminal settings found.");
  });

  it("terminal help says a running program may not notice", () => {
    expect(html()).toContain("A running program may not notice the change until it redraws or restarts.");
  });

  it("shows where the terminal font comes from", () => {
    expect(html()).toContain("Bundled: JetBrains Mono");
    const wt = { path: "x", text: '{ "profiles": { "defaults": { "font": { "face": "Cascadia Mono" } } } }' };
    expect(html({ wtFiles: [wt] })).toContain("From Windows Terminal: Cascadia Mono");
  });

  it("a failed save shows next to its section", () => {
    const m = html({ error: { at: "terminal", text: "Couldn't save the appearance settings: the core didn't answer." } });
    // Shown in words beside the Terminal section; the dialog says it aloud through the window's announcer, not a region of its own.
    expect(m).not.toContain('role="alert"');
    expect(m).toMatch(/Terminal font size(?:(?!<h3).)*the core didn&#x27;t answer/s);
  });
});

describe("appearance edits", () => {
  it("removing_current_theme_resets_setting", () => {
    const a = { ...DEFAULT_APPEARANCE, terminal_theme: "nord", imported_themes: [theme("nord"), theme("dracula")] };
    const out = removeImported(a, "nord");
    expect(out.imported_themes.map((t) => t.id)).toEqual(["dracula"]);
    expect(out.terminal_theme).toBe("follow-app");
    expect(removeImported({ ...a, terminal_theme: "dark" }, "nord").terminal_theme).toBe("dark");
  });

  it("adding a theme whose id is taken gives it a new id", () => {
    const a = { ...DEFAULT_APPEARANCE, imported_themes: [theme("nord")] };
    expect(addImported(a, [theme("nord", "Nord again"), theme("dracula")]).imported_themes.map((t) => t.id)).toEqual(["nord", "nord-2", "dracula"]);
  });
});
