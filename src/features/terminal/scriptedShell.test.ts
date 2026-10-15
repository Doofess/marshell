import { describe, expect, it } from "vitest";
import { SHELL_SIZE, shellSession } from "./scriptedShell";

const plain = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "");
const lines = (s: string) => plain(s).split("\r\n");

describe("the scripted plain shell", () => {
  it("fits the sidebar: 34 columns by 9 rows", () => {
    expect(SHELL_SIZE).toEqual({ cols: 34, rows: 9 });
  });
  it("is a PowerShell session by default, in the project's folder, ending at a fresh prompt", () => {
    const s = shellSession({ cwd: "my-app" });
    const l = lines(s);
    expect(l[0]).toBe("PS C:\\dev\\my-app> git status -s");
    expect(l).toContain("PS C:\\dev\\my-app> npm test");
    expect(l.at(-1)).toBe("PS C:\\dev\\my-app> ");
  });
  it("leaves the cursor on the last prompt: no newline after it", () => {
    expect(shellSession({ cwd: "my-app" }).endsWith("PS C:\\dev\\my-app> ")).toBe(true);
  });
  it("never wraps: every line is at most 34 columns, and there are at most 9", () => {
    for (const shell of ["PowerShell", "zsh", "bash"] as const)
      for (const cwd of ["my-app", "x", "a-very-long-project-name-indeed", "terraform"]) {
        const l = lines(shellSession({ shell, cwd }));
        expect(l.length, `${shell} ${cwd}`).toBeLessThanOrEqual(SHELL_SIZE.rows);
        for (const line of l) expect([...line].length, `${shell} ${cwd}: ${line}`).toBeLessThanOrEqual(SHELL_SIZE.cols);
      }
  });
  it("shortens a long folder name with an ellipsis rather than wrapping", () => {
    const s = plain(shellSession({ cwd: "a-very-long-project-name-indeed" }));
    expect(s).toContain("…");
    expect(s).not.toContain("indeed");
  });
  it("speaks each shell's own prompt", () => {
    expect(lines(shellSession({ shell: "zsh", cwd: "my-app" }))[0]).toMatch(/^my-app % git status -s$/);
    expect(lines(shellSession({ shell: "bash", cwd: "my-app" }))[0]).toMatch(/^my-app\$ git status -s$/);
  });
  it("uses carriage-return line feeds, as a terminal needs", () => {
    expect(shellSession({ cwd: "my-app" })).toContain("\r\n");
    expect(shellSession({ cwd: "my-app" })).not.toMatch(/[^\r]\n/);
  });
  it("says nothing about any agent: it is a plain shell", () => {
    expect(plain(shellSession({ cwd: "my-app" })).toLowerCase()).not.toMatch(/claude|codex|antigravity|agent/);
  });
});
