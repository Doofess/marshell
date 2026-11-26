import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (p: string) => readFileSync(new URL(`../../${p}`, import.meta.url), "utf8");
const hooks = read("src-tauri/windows/hooks.nsh");

/** The programs the installer puts in the install folder: the app, and every bundled resource that is an exe or a dll. */
function installedPrograms(): string[] {
  const names = ["${MAINBINARYNAME}.exe"];
  for (const conf of ["src-tauri/tauri.windows.conf.json", "src-tauri/tauri.release.conf.json"]) {
    const resources = JSON.parse(read(conf)).bundle.resources as Record<string, string>;
    names.push(...Object.values(resources).filter((target) => /\.(exe|dll)$/i.test(target)));
  }
  return names.sort();
}

/** The lines of one macro in the hooks file, without its comments. */
function macroBody(name: string): string[] {
  const m = hooks.match(new RegExp(`^!macro ${name}\\b.*\\r?\\n([\\s\\S]*?)^!macroend`, "m"));
  if (!m) return [];
  return m[1].split(/\r?\n/).map((l) => l.trim()).filter((l) => l && !l.startsWith(";"));
}

describe("the uninstaller's wait for programs that are still ending (issue #212)", () => {
  it("tries again for every program the installer puts in the install folder", () => {
    const named = macroBody("MARSHELL_REMOVE_HELD_PROGRAMS")
      .map((l) => l.match(/^!insertmacro MARSHELL_DELETE_PROGRAM "([^"]+)"$/)?.[1])
      .filter((n): n is string => !!n)
      .sort();
    expect(named).toEqual(installedPrograms());
  });

  it("gives up after 20 seconds unless a build says otherwise", () => {
    expect(hooks).toMatch(/^!ifndef MARSHELL_HELD_WAIT_MS\r?\n\s*!define MARSHELL_HELD_WAIT_MS 20000\r?\n!endif/m);
  });

  it("runs in the uninstaller's last hook whatever started the uninstall, an update included", () => {
    let depth = 0;
    let found = false;
    for (const line of macroBody("NSIS_HOOK_POSTUNINSTALL")) {
      if (/^\$\{If\}/.test(line)) depth++;
      else if (/^\$\{EndIf\}/.test(line)) depth--;
      else if (line === "!insertmacro MARSHELL_REMOVE_HELD_PROGRAMS") found = depth === 0;
    }
    expect(found).toBe(true);
  });
});
