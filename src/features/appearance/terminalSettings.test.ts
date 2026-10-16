import { describe, expect, it } from "vitest";
import { resolveTerminalTheme } from "../terminal/terminalTheme";
import { DEFAULT_APPEARANCE } from "./rootState";
import { terminalThemeOf, withColorScheme, wtFont } from "./terminalSettings";

const WT = (font: string) => ({ path: "C:/wt/settings.json", text: `{ "profiles": { "defaults": { ${font} } } }` });

describe("terminalThemeOf", () => {
  it("carries the setting, imported themes, protection and the profile's override", () => {
    const a = { ...DEFAULT_APPEARANCE, terminal_theme: "dark", contrast_protection: false };
    expect(terminalThemeOf(a, "light")).toEqual({ setting: "dark", override: "light", imported: [], protection: false });
    expect(terminalThemeOf(a, undefined).override).toBeNull();
  });
  it("a light profile override starts a light session in a dark app", () => {
    const input = { ...terminalThemeOf(DEFAULT_APPEARANCE, "light"), app: "dark" as const };
    expect(resolveTerminalTheme(input).scheme).toBe("light");
  });
});

describe("withColorScheme", () => {
  const req = { cols: 80, rows: 24, profile_id: "pwsh" };
  const profile = (terminal_theme?: string) => ({ id: "pwsh", name: "PowerShell", command: ["pwsh"], family: "power_shell" as const, cwd: null, env: [], shell_integration: true, detected: false, terminal_theme });
  it("tells the core the scheme the session's terminal will show", () => {
    expect(withColorScheme(req, [profile("light")], DEFAULT_APPEARANCE, "dark").color_scheme).toBe("light");
    expect(withColorScheme(req, [profile()], DEFAULT_APPEARANCE, "dark").color_scheme).toBe("dark");
  });
  it("an unknown profile follows the setting", () => {
    expect(withColorScheme(req, [], { ...DEFAULT_APPEARANCE, terminal_theme: "light" }, "dark").color_scheme).toBe("light");
  });
});

describe("wtFont", () => {
  it("takes the first file that names a font", () => {
    expect(wtFont([WT(""), WT('"font": { "face": "Cascadia Code", "size": 11 }')])).toEqual({ family: "Cascadia Code", size: 11 });
  });
  it("skips a file that isn't valid JSON", () => {
    expect(wtFont([{ path: "x", text: "{ nope" }, WT('"fontFace": "Consolas"')])).toEqual({ family: "Consolas" });
  });
  it("nothing found is null", () => {
    expect(wtFont([])).toBeNull();
    expect(wtFont([WT("")])).toBeNull();
  });
});
