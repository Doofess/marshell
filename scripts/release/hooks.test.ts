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

/** An NSIS folder variable other than $INSTDIR, matched as a whole name. */
const OTHER_FOLDER =
  /\$(?:TEMP|PLUGINSDIR|EXEDIR|EXEPATH|OUTDIR|WINDIR|SYSDIR|PROGRAMFILES(?:32|64)?|COMMONFILES(?:32|64)?|APPDATA|LOCALAPPDATA|PROFILE|DESKTOP|DOCUMENTS|SMPROGRAMS|STARTMENU|SMSTARTUP|SENDTO|RESOURCES|FONTS)(?![A-Za-z0-9_])/;
const DRIVE_PATH = /[A-Za-z]:[\\/]/;

/**
 * The first line of a macro body that reaches outside the install folder or looks at a process, or null. Comment
 * lines are skipped. NSIS escapes (a dollar sign, a backslash and one character) are dropped, and then every
 * `$INSTDIR` followed by a backslash is taken out: a backslash that is left belongs to some other path, however it
 * is quoted. Another folder variable, a drive letter path and `..` count as well. A plain `/` doesn't, since
 * `MessageBox ... /SD IDCANCEL` and `!define /math` use it.
 */
function onlyInstallFolder(body: string[]): string | null {
  const process = /nsExec|nsProcess|FindProcess|KillProcess|ExecWait|ExecShell|^\s*Exec\b|RmShutdown|RmGetList|RmStartSession|nsis_tauri_utils|taskkill|OpenProcess|TerminateProcess|CreateToolhelp32Snapshot|Process32|EnumProcesses/i;
  for (const line of body) {
    const code = line.trim();
    if (code.startsWith(";") || code.startsWith("#")) continue;
    const rest = code.replace(/\$\\./g, "").split("$INSTDIR\\").join("");
    // Another folder variable, a drive letter path, or a step up out of the folder.
    if (rest.includes("\\") || process.test(code) || OTHER_FOLDER.test(rest) || DRIVE_PATH.test(rest) || rest.includes("..")) return line;
  }
  return null;
}

const WAIT = "!insertmacro MARSHELL_REMOVE_HELD_PROGRAMS";

/**
 * Whether the wait is a macro's first line, and its only one. First, so that nothing can come before it: not a
 * condition, not a jump, and not the step that takes the hooks out, which can take minutes while an install that
 * follows the uninstall writes the same files.
 */
function runsFirstAndOnce(body: string[]): boolean {
  return body[0] === WAIT && body.filter((line) => line.includes("MARSHELL_REMOVE_HELD_PROGRAMS")).length === 1;
}

describe("the installer's and the uninstaller's waits for programs that are still ending (issues #212 and #214)", () => {
  it("names every program the installer puts in the install folder, once", () => {
    const named = macroBody("MARSHELL_EACH_PROGRAM")
      .map((l) => l.match(/^!insertmacro \$\{do\} "([^"]+)"$/)?.[1])
      .filter((n): n is string => !!n)
      .sort();
    expect(named).toEqual(installedPrograms());
  });

  it("goes through that list in both waits, and names no program itself", () => {
    expect(macroBody("MARSHELL_REMOVE_HELD_PROGRAMS")).toContain("!insertmacro MARSHELL_EACH_PROGRAM MARSHELL_DELETE_PROGRAM");
    expect(macroBody("MARSHELL_WAIT_FOR_HELD_PROGRAMS")).toContain("!insertmacro MARSHELL_EACH_PROGRAM MARSHELL_PROBE_PROGRAM");
    const naming = hooks.split(/\r?\n/).filter((line) => /!insertmacro MARSHELL_(DELETE|PROBE)_PROGRAM "/.test(line));
    expect(naming).toEqual([]);
  });

  it("closes the app the template's way in the installer's first hook, then waits, and does nothing else first", () => {
    expect(macroBody("NSIS_HOOK_PREINSTALL")).toEqual([
      '!insertmacro CheckIfAppIsRunning "$INSTDIR\\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"',
      "!insertmacro MARSHELL_WAIT_FOR_HELD_PROGRAMS",
    ]);
  });

  it("looks only at files in the install folder, never at a process", () => {
    for (const name of [
      "MARSHELL_EACH_PROGRAM",
      "MARSHELL_DELETE_PROGRAM",
      "MARSHELL_PROBE_PROGRAM",
      "MARSHELL_REMOVE_HELD_PROGRAMS",
      "MARSHELL_WAIT_FOR_HELD_PROGRAMS",
    ]) {
      const body = macroBody(name);
      expect(body.length, `${name} has a body`).toBeGreaterThan(0);
      expect(onlyInstallFolder(body), name).toBeNull();
    }
  });

  it("would notice a path outside the install folder, or a look at a process", () => {
    for (const bad of [
      'Delete "$TEMP\\x.exe"',
      "Delete $TEMP\\x.exe",
      "Delete `C:\\x`",
      "System::Call 'kernel32::OpenProcess(i 1, i 0, i r0) p.r1'",
      "nsExec::Exec 'taskkill'",
      'nsis_tauri_utils::FindProcess "OpenConsole.exe"',
      'Exec "x"',
      'Delete "$TEMP"',
      'RMDir "$PLUGINSDIR"',
      'Delete "C:/x.exe"',
      'Delete "$INSTDIR/../x.exe"',
      'CopyFiles "$EXEDIR\\x" "$INSTDIR\\x"',
    ]) {
      expect(onlyInstallFolder([bad]), bad).toBe(bad);
    }
    for (const good of [
      'FileWrite $R6 "$R5$\\r$\\n"',
      'Delete "$INSTDIR\\${file}"',
      `System::Call 'kernel32::CreateFileW(w "$INSTDIR\\\${file}", i 0x40000000, i 1, p 0, i 3, i 0x80, p 0) p.R7 ?e'`,
      `System::Call 'kernel32::GetFileAttributesW(w "$INSTDIR\\\${file}") i.R4'`,
      `System::Call 'kernel32::SetFileAttributesW(w "$INSTDIR\\\${file}", i R5)'`,
      'MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "x$\\n$\\n$R4" /SD IDCANCEL IDRETRY',
    ]) {
      expect(onlyInstallFolder([good]), good).toBeNull();
    }
  });

  it("says what the person is told (PLAN §9, slice 9d decision 3)", () => {
    const secs = "${MARSHELL_HELD_WAIT_S}";
    for (const text of [
      `Waiting for Marshell's programs to end (up to ${secs} seconds)`,
      "Marshell has closed, but its program $R9 is still running, so the installer can't replace it.$\\n$\\n$R4",
      "Retry waits for it again. Cancel stops the installer and changes nothing.",
      "Retry waits for it again. Cancel stops the installer; the earlier version has already been removed.",
      `Marshell wasn't updated: $R9 was still running ${secs} seconds after Marshell closed. Nothing was changed. Run the update again.`,
      `Marshell wasn't installed: $R9 was still running ${secs} seconds after Marshell closed. The earlier version had already been removed. Run the installer again.`,
    ]) {
      expect(hooks, text).toContain(text);
    }
    expect(hooks).not.toContain("Waiting for Marshell's programs to end\"");
    // The details line is in both waits.
    const detail = `DetailPrint "Waiting for Marshell's programs to end (up to ${secs} seconds)"`;
    expect(macroBody("MARSHELL_REMOVE_HELD_PROGRAMS")).toContain(detail);
    expect(macroBody("MARSHELL_WAIT_FOR_HELD_PROGRAMS")).toContain(detail);
  });

  it("takes the copy's seconds from the limit", () => {
    expect(hooks).toMatch(/^!define \/math MARSHELL_HELD_WAIT_S \$\{MARSHELL_HELD_WAIT_MS\} \/ 1000$/m);
  });

  it("exits 32 when a stop gives up, and only then", () => {
    expect(hooks).toMatch(/^!define MARSHELL_HELD_EXIT 32$/m);
    const body = macroBody("MARSHELL_WAIT_FOR_HELD_PROGRAMS");
    const aborts = body.map((line, i) => (/\bAbort\b/.test(line) ? i : -1)).filter((i) => i >= 0);
    expect(aborts).toHaveLength(2);
    for (const i of aborts) expect(body[i - 1]).toBe("SetErrorLevel ${MARSHELL_HELD_EXIT}");
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

  it("runs first in the uninstaller's last hook, before the hooks are taken out, whatever started the uninstall", () => {
    expect(runsFirstAndOnce(macroBody("NSIS_HOOK_POSTUNINSTALL"))).toBe(true);
  });

  it("would notice the wait put behind anything, under a condition, or run a second time", () => {
    expect(runsFirstAndOnce([WAIT, "${If} $UpdateMode <> 1", "DeleteRegKey HKCU x", "${EndIf}"])).toBe(true);
    expect(runsFirstAndOnce(["nsExec::ExecToLog /TIMEOUT=120000 'x uninstall --yes'", WAIT])).toBe(false);
    expect(runsFirstAndOnce(["${If} $UpdateMode <> 1", WAIT, "${EndIf}"])).toBe(false);
    expect(runsFirstAndOnce(["${IfNot} ${Silent}", WAIT, "${EndIf}"])).toBe(false);
    expect(runsFirstAndOnce([`\${IfThen} $UpdateMode <> 1 \${|} ${WAIT} \${|}`])).toBe(false);
    expect(runsFirstAndOnce(["IfSilent +2", WAIT])).toBe(false);
    expect(runsFirstAndOnce(["!if 1", WAIT, "!endif"])).toBe(false);
    expect(runsFirstAndOnce([WAIT, "DetailPrint x", WAIT])).toBe(false);
    expect(runsFirstAndOnce([])).toBe(false);
  });
});
