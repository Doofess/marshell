import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (p: string) => readFileSync(new URL(`../../${p}`, import.meta.url), "utf8");
const hooks = read("src-tauri/windows/hooks.nsh");

type BundleConf = { bundle?: { resources?: Record<string, string> | string[]; externalBin?: string[] } };

/**
 * The programs a set of Tauri configs puts in the install folder: the app, every bundled resource that is an exe or
 * a dll (by its target name, or by its own path when the resources are a list), and every sidecar (`externalBin`,
 * installed under its file name).
 */
function programsOf(configs: BundleConf[]): string[] {
  const names = ["${MAINBINARYNAME}.exe"];
  for (const conf of configs) {
    const resources = conf.bundle?.resources ?? {};
    // With backslashes, the way the hook names a file under $INSTDIR.
    names.push(...Object.values(resources).filter((target) => /\.(exe|dll)$/i.test(target)).map((target) => target.replace(/\//g, "\\")));
    names.push(...(conf.bundle?.externalBin ?? []).map((bin) => `${bin.split(/[\\/]/).pop()}.exe`));
  }
  return names.sort();
}

const CONFIGS = ["src-tauri/tauri.conf.json", "src-tauri/tauri.windows.conf.json", "src-tauri/tauri.release.conf.json"];
const installedPrograms = () => programsOf(CONFIGS.map((conf) => JSON.parse(read(conf)) as BundleConf));

/** The lines of one macro in the hooks file, without its comments. */
function macroBody(name: string): string[] {
  const m = hooks.match(new RegExp(`^!macro ${name}\\b.*\\r?\\n([\\s\\S]*?)^!macroend`, "m"));
  if (!m) return [];
  return m[1].split(/\r?\n/).map((l) => l.trim()).filter((l) => l && !l.startsWith(";"));
}

const WAIT = "!insertmacro MARSHELL_REMOVE_HELD_PROGRAMS";

/** Whether a macro's lines end with the wait, with nothing before it that could skip it. */
function runsLastAndAlways(body: string[]): boolean {
  if (body.length === 0 || body[body.length - 1] !== WAIT) return false;
  let depth = 0;
  for (const line of body.slice(0, -1)) {
    // A jump or a compile-time condition before it could pass it by.
    if (/^(Goto|Return|Abort|Quit)\b|^!(if|ifdef|ifndef|else|endif)\b/i.test(line)) return false;
    if (/^\$\{(If|IfNot|Unless|Do|DoWhile|DoUntil|For|ForEach|While|Select|Switch)\}/.test(line)) depth++;
    else if (/^\$\{(EndIf|EndUnless|Loop|LoopWhile|LoopUntil|Next|EndWhile|EndSelect|EndSwitch)\}/.test(line)) depth--;
  }
  return depth === 0;
}

describe("the uninstaller's wait for programs that are still ending (issue #212)", () => {
  it("tries again for every program the installer puts in the install folder", () => {
    const named = macroBody("MARSHELL_REMOVE_HELD_PROGRAMS")
      .map((l) => l.match(/^!insertmacro MARSHELL_DELETE_PROGRAM "([^"]+)"$/)?.[1])
      .filter((n): n is string => !!n)
      .sort();
    expect(named).toEqual(installedPrograms());
  });

  it("counts a program however a config bundles it: a resource by its target name, a sidecar by its file name", () => {
    expect(
      programsOf([
        { bundle: { resources: { "staged/a.exe": "a.exe", "x/b.DLL": "lib/b.DLL", "../LICENSE": "LICENSE" } } },
        { bundle: { resources: ["res/c.exe", "res/readme.txt"], externalBin: ["binaries/helper", "tools\\probe"] } },
        {},
      ]),
    ).toEqual(["${MAINBINARYNAME}.exe", "a.exe", "helper.exe", "lib\\b.DLL", "probe.exe", "res\\c.exe"]);
  });

  it("gives up after 20 seconds unless a build says otherwise", () => {
    expect(hooks).toMatch(/^!ifndef MARSHELL_HELD_WAIT_MS\r?\n\s*!define MARSHELL_HELD_WAIT_MS 20000\r?\n!endif/m);
  });

  it("runs in the uninstaller's last hook whatever started the uninstall, an update included", () => {
    expect(runsLastAndAlways(macroBody("NSIS_HOOK_POSTUNINSTALL"))).toBe(true);
  });

  it("would notice the wait put under a condition, or no longer last", () => {
    expect(runsLastAndAlways(["${If} $UpdateMode <> 1", "DeleteRegKey HKCU x", "${EndIf}", WAIT])).toBe(true);
    expect(runsLastAndAlways(["${If} $UpdateMode <> 1", WAIT, "${EndIf}"])).toBe(false);
    expect(runsLastAndAlways(["${IfNot} ${Silent}", WAIT, "${EndIf}"])).toBe(false);
    expect(runsLastAndAlways(["${Unless} $UpdateMode = 1", WAIT, "${EndUnless}"])).toBe(false);
    expect(runsLastAndAlways([`\${IfThen} $UpdateMode <> 1 \${|} ${WAIT} \${|}`])).toBe(false);
    expect(runsLastAndAlways(["!if 1", WAIT, "!endif"])).toBe(false);
    expect(runsLastAndAlways(["Goto done", WAIT])).toBe(false);
    expect(runsLastAndAlways(["Return", WAIT])).toBe(false);
    expect(runsLastAndAlways([WAIT, "DetailPrint x"])).toBe(false);
    expect(runsLastAndAlways([])).toBe(false);
  });
});
