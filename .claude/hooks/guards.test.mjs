// Tests for the PreToolUse guard hooks. Each case runs the hook as a child process with a JSON stdin.
import { test, describe, before, after } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WIN = process.platform === "win32";

// ---- reasons, copied verbatim from the plan's tables -------------------------------------------------------------
const R = {
  S1: "Stop processes by PID only, after listing the tree: real agent CLIs run on this machine (SCOPE.md).",
  S2: "Don't delete a variable-built path recursively. Use a literal scratch path, and ask before cleaning scratch folders.",
  S3: 'The stash is shared with other sessions. Use a WIP commit, or git stash push -u -m "<tag>" and apply by SHA.',
  S3D: "The stash is shared by every session and worktree, so clear or an unnamed drop can destroy another session's work. Drop a stash by its SHA, and only your own.",
  S4: "Credential files and the real Marshell token are off limits (SCOPE.md). A live check reads its scratch home's token.",
  S5: "Don't name a test binary like a real CLI. Use a name like codex-marshell-test.exe.",
  S6: "A live check runs with MARSHELL_HOME, CLAUDE_CONFIG_DIR and MARSHELL_CLAUDE_SETTINGS set to scratch folders. See the marshell-live-check skill.",
  S7: "This can't be undone. Confirm with the user first.",
  S8: "Run cargo fmt --all before a Rust commit.",
  F1: "src/generated is written by ts-rs. Change crates/protocol and run cargo test -p marshell-protocol.",
  F2: "This is an owner file. Propose the change to the user instead.",
  F3: "Capabilities, the CSP and the API guard change only with the user's word (SCOPE.md).",
  F4: "Real CLI configuration and Marshell data are off limits while developing (SCOPE.md).",
};

// ---- helpers -----------------------------------------------------------------------------------------------------
const created = [];
function tmp(prefix) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  created.push(dir);
  return dir;
}
after(() => {
  for (const dir of created) fs.rmSync(dir, { recursive: true, force: true });
});

// A fake home, so the tests never look at or touch the real one.
const FAKE_HOME = tmp("guards-home-");
const FAKE_HOME_ENV = { USERPROFILE: FAKE_HOME, HOME: FAKE_HOME };

/** process.env with every case variant of PATH removed, then PATH set to `value`. */
function envWithPath(value) {
  const env = {};
  for (const [k, v] of Object.entries(process.env)) if (k.toLowerCase() !== "path") env[k] = v;
  env.PATH = value;
  return env;
}

function run(hook, stdin, { env, realHome } = {}) {
  const childEnv = env ?? { ...process.env, ...(realHome ? {} : FAKE_HOME_ENV) };
  const r = spawnSync(process.execPath, [path.join(HERE, hook)], {
    input: stdin,
    encoding: "utf8",
    env: childEnv,
    timeout: 60000,
  });
  return { status: r.status, stdout: r.stdout ?? "", stderr: r.stderr ?? "" };
}

function shell(command, { cwd = FAKE_HOME, tool = "Bash", env, realHome } = {}) {
  return run("guard-shell.mjs", JSON.stringify({ tool_name: tool, tool_input: { command }, cwd }), { env, realHome });
}
const ps = (command, opts = {}) => shell(command, { ...opts, tool: "PowerShell" });

function edit(file, { tool = "Edit", key = "file_path", cwd, env } = {}) {
  return run("guard-files.mjs", JSON.stringify({ tool_name: tool, tool_input: { [key]: file }, cwd }), { env });
}

function decision(r, kind, reason) {
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(JSON.parse(r.stdout), {
    hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision: kind, permissionDecisionReason: reason },
  });
}
const denied = (r, reason) => decision(r, "deny", reason);
const asked = (r, reason) => decision(r, "ask", reason);
function none(r) {
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "");
}

const GIT_ID = ["-c", "user.name=t", "-c", "user.email=t@t"];
function git(dir, ...args) {
  const r = spawnSync("git", [...GIT_ID, "-C", dir, ...args], { encoding: "utf8" });
  assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout;
}

/** A temp git repo. With `marshell`, it has crates/protocol and src-tauri, which makes it look like a checkout of this repo. */
function mkRepo({ marshell = true } = {}) {
  const dir = tmp("guards-repo-");
  git(dir, "init", "-q");
  if (marshell) {
    for (const d of ["crates/protocol", "src-tauri"]) {
      fs.mkdirSync(path.join(dir, d), { recursive: true });
      fs.writeFileSync(path.join(dir, d, ".gitkeep"), "");
    }
  }
  return dir;
}

function writeAndStage(dir, rel, content) {
  const file = path.join(dir, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
  git(dir, "add", rel);
}

const fwd = (p) => p.replace(/\\/g, "/");

/** The 8.3 short form of an existing path (Windows), or null when there is none. */
function shortPath(p) {
  if (!WIN) return null;
  const r = spawnSync(`for %I in ("${p}") do @echo %~sI`, { shell: true, encoding: "utf8" });
  const s = (r.stdout ?? "").trim();
  return r.status === 0 && s && s.toLowerCase() !== p.toLowerCase() ? s : null;
}
const SHORT_HOME = shortPath(FAKE_HOME);
const NO_SHORT = !SHORT_HOME && "no 8.3 short name for the temp home on this machine";

// cargo is needed for the S8 tests that run `cargo fmt`, with the same env the hook child gets (the real home).
const HAS_CARGO = spawnSync("cargo", ["fmt", "--version"], { env: process.env, encoding: "utf8" }).status === 0;

// ---- fail open (Review Focus 2) ----------------------------------------------------------------------------------
describe("fail open", () => {
  const inputs = [
    ["empty stdin", ""],
    ["non-JSON stdin", "nope"],
    ["JSON array", "[]"],
    ["empty object", "{}"],
    ["no tool_input (Bash)", '{"tool_name":"Bash"}'],
    ["no tool_input (Edit)", '{"tool_name":"Edit"}'],
    ["empty tool_input (Bash)", '{"tool_name":"Bash","tool_input":{}}'],
    ["empty tool_input (Write)", '{"tool_name":"Write","tool_input":{}}'],
    ["non-string command", '{"tool_name":"Bash","tool_input":{"command":123}}'],
    ["non-string file_path", '{"tool_name":"Write","tool_input":{"file_path":42}}'],
    ["unknown tool", '{"tool_name":"Read","tool_input":{"file_path":"SCOPE.md","command":"git stash"}}'],
  ];
  for (const hook of ["guard-shell.mjs", "guard-files.mjs"]) {
    for (const [name, stdin] of inputs) {
      test(`${hook}: ${name}`, () => none(run(hook, stdin)));
    }
  }
});

// ---- S1 ----------------------------------------------------------------------------------------------------------
describe("S1: killing by name", () => {
  const bad = [
    "taskkill /IM claude.exe /F",
    "taskkill /f /im codex.exe",
    "taskkill.exe //IM node.exe",
    "pkill -f claude",
    "killall node",
    "cd x && taskkill /IM foo.exe",
    "env FOO=1 pkill node",
    "FOO=1 killall node",
  ];
  for (const c of bad) test(`Bash: ${c}`, () => denied(shell(c), R.S1));
  const badPs = [
    "Stop-Process -Name node -Force",
    "Stop-Process -ProcessName claude",
    "Get-Process claude | Stop-Process",
    "& taskkill /IM codex.exe",
    "$p = 1; stop-process -name node",
    "Get-Process claude | Where-Object { $_.Id -ne 1 } | Stop-Process -Force",
    "Get-Process claude | ForEach-Object { $_.Kill() }",
    "kill -Name claude",
    "spps -Name claude",
    "Stop-Process -N claude",
    "stop-process -na claude",
    "(Get-Process codex).Kill()",
  ];
  for (const c of badPs) test(`PowerShell: ${c}`, () => denied(ps(c), R.S1));

  const ok = [
    "taskkill /PID 123 /T /F",
    "Stop-Process -Id 123",
    'echo "Ruling: taskkill /IM is denied" >> progress.md',
    'grep -n "pkill" docs/x.md',
    "git log --grep=taskkill",
    "Get-Process -Id 4242 | Stop-Process -Force",
    "Stop-Process -Id 4242",
    "Get-Process -Name claude",
    "kill -n 9 123",
  ];
  for (const c of ok) test(`no decision: ${c}`, () => none(shell(c)));
});

// ---- S2 ----------------------------------------------------------------------------------------------------------
describe("S2: variable-built recursive delete", () => {
  const bad = [
    'rm -rf "$TMP/mh"',
    "rm -fr $dir",
    "rm -r $HOME/x",
    "rm -rf %TEMP%\\mh",
    "rm -rf \x60pwd\x60/x",
    "rm -Rf $(pwd)/x",
    "ls && rm -rf $dir",
    'for d in a b; do rm -rf "$d"; done',
  ];
  for (const c of bad) test(`Bash: ${c}`, () => denied(shell(c), R.S2));
  const badPs = [
    'Remove-Item -Recurse -Force "$env:TEMP\\mh$id"',
    "Remove-Item $x -Recurse",
    "Remove-Item -Path C:\\Users\\x\\$dir -Recurse -Force",
    "rm -r -force $x",
    "ri -r $dir",
    "rmdir -Recurse -Force $dir",
    "del -Recurse $env:TEMP/x",
    "erase -Recurse $x",
    "rd -Recurse $x",
    "try { Remove-Item -Recurse $x } catch {}",
  ];
  for (const c of badPs) test(`PowerShell: ${c}`, () => denied(ps(c), R.S2));

  const ok = [
    "rm -rf C:/Users/x/AppData/Local/Temp/claude/scratch/mh1",
    "rm -f $x",
    "rm file.txt",
    "rm -d $x",
  ];
  for (const c of ok) test(`no decision: ${c}`, () => none(shell(c)));
  const okPs = [
    "Remove-Item -Recurse -Force C:\\Users\\x\\AppData\\Local\\Temp\\mh1",
    "Remove-Item -Force $x",
  ];
  for (const c of okPs) test(`no decision (PowerShell): ${c}`, () => none(ps(c)));
});

// ---- S3 ----------------------------------------------------------------------------------------------------------
describe("S3: git stash", () => {
  const bad = [
    "git stash",
    "git stash pop",
    "git stash save wip",
    "git stash push",
    "git stash push -u",
    "git stash -u",
    "git stash push -- src/a.ts",
    "git -C C:\\dev\\x stash",
    "git stash; echo done",
    "git.exe stash pop",
    "if true; then git stash pop; fi",
    "time git stash pop",
    "nohup git stash pop",
    "while true; do git stash; done",
    "! git stash pop",
    "{ git stash pop; }",
  ];
  for (const c of bad) test(`Bash: ${c}`, () => denied(shell(c), R.S3));
  test("PowerShell: git stash pop", () => denied(ps("git stash pop"), R.S3));
  test("PowerShell: & git stash", () => denied(ps("& git stash"), R.S3));
  test("PowerShell: try { git stash pop } catch {}", () => denied(ps("try { git stash pop } catch {}"), R.S3));
  test("PowerShell: } catch { git stash pop }", () => denied(ps("try {\n  foo\n} catch { git stash pop }"), R.S3));
  test("PowerShell: } finally { git stash pop }", () => denied(ps("try {\n  foo\n} finally { git stash pop }"), R.S3));
  for (const c of ["git stash clear", "git stash drop", "git stash drop -q", "git -C x stash clear"]) {
    test(`Bash: ${c}`, () => denied(shell(c), R.S3D));
  }

  const ok = [
    "git stash list",
    "git stash apply 1a2b3c",
    'git stash push -u -m "wip-x"',
    "git stash push --message wip",
    "git stash drop stash@{2}",
    "git stash drop 1a2b3c4d",
    "git stash drop -q stash@{0}",
    "git stash show -p",
  ];
  for (const c of ok) test(`no decision: ${c}`, () => none(shell(c)));
});

// ---- S4 ----------------------------------------------------------------------------------------------------------
describe("S4: credential files and the real token", () => {
  const bad = [
    "cat ~/.claude/.credentials.json",
    "cat C:\\Users\\x\\.claude\\.credentials.json",
    "cat ~/.codex/auth.json",
    "cat ~/.marshell/run/endpoint.json",
    "cat .env",
    'cat "C:/Program Files/app/.env"',
    "cat .env.local",
    "grep token .env",
    "Get-Content $HOME/.codex/auth.json",
    "cp ~/.codex/auth.json x",
    "type .env",
    "grep -e token .env",
    "rg -g '*.md' token .env",
    "bash -c 'cat ~/.claude/.credentials.json'",
    "echo $(cat .env)",
  ];
  for (const c of bad) test(`Bash: ${c}`, () => denied(shell(c), R.S4));
  const badPs = [
    "Get-Content $env:USERPROFILE\\.codex\\auth.json",
    "Get-Content $env:USERPROFILE\\.marshell\\run\\endpoint.json",
    "type %USERPROFILE%\\.marshell\\run\\endpoint.json",
    "Get-Content .\\.env",
    "Get-Content -Path:C:\\x\\.credentials.json",
    "Select-String token .env",
    "Select-String -Path .env -Pattern token",
    "sls -Pattern token -LiteralPath $HOME/.codex/auth.json",
  ];
  for (const c of badPs) test(`PowerShell: ${c}`, () => denied(ps(c), R.S4));

  const ok = [
    "Get-Content $env:TEMP\\mh\\run\\endpoint.json",
    "cat /tmp/mh/run/endpoint.json",
    "cat .env.example",
    "cat envfile.txt",
    'git commit -m "docs: .env and .credentials.json are off limits"',
    'grep -rn ".marshell/run/endpoint.json" docs/',
    'rg -n "~/.codex/auth.json" docs',
    'grep -rn "\\.env" .gitignore',
    "git log --oneline -- .env",
    'echo "Never read ~/.claude/.credentials.json" >> notes.md',
    "cat .env.local.example",
    "rg -e '.credentials.json' docs",
    "grep -A 3 -m 2 '.env' README.md",
  ];
  for (const c of ok) test(`no decision: ${c}`, () => none(shell(c)));
  const okPs = [
    'Select-String -Pattern "auth.json" -Path docs/x.md',
    "Select-String '.env' README.md",
    "Write-Host 'never read .credentials.json'",
  ];
  for (const c of okPs) test(`no decision (PowerShell): ${c}`, () => none(ps(c)));
});

// ---- S5 ----------------------------------------------------------------------------------------------------------
describe("S5: test binaries named like real CLIs", () => {
  const bad = [
    "cp build/out.exe /tmp/codex.exe",
    "mv a.exe C:\\t\\claude.exe",
    "cp C:/real/codex.exe other.exe",
    "copy a.exe codex.exe",
    "move a.exe C:\\t\\Claude.EXE",
  ];
  for (const c of bad) test(`Bash: ${c}`, () => denied(shell(c), R.S5));
  const badPs = [
    "Copy-Item build\\out.exe C:\\t\\claude.exe",
    'Move-Item x "C:\\t\\codex.exe"',
    "Rename-Item a.exe codex.exe",
    "Copy-Item a.exe -Destination:C:\\t\\codex.exe",
  ];
  for (const c of badPs) test(`PowerShell: ${c}`, () => denied(ps(c), R.S5));

  const ok = ["cp a.exe codex-marshell-test.exe", "ls codex.exe", "echo codex.exe", "cp a b"];
  for (const c of ok) test(`no decision: ${c}`, () => none(shell(c)));
});

// ---- S6 ----------------------------------------------------------------------------------------------------------
describe("S6: tauri dev outside a scratch environment", () => {
  const bad = [
    "pnpm tauri dev",
    "cargo tauri dev",
    "pnpm exec tauri dev",
    "npx tauri dev",
    "MARSHELL_HOME=/tmp/mh pnpm tauri dev",
    "CLAUDE_CONFIG_DIR=/tmp/cc pnpm tauri dev",
    "MARSHELL_HOME=~/.marshell CLAUDE_CONFIG_DIR=/tmp/cc pnpm tauri dev",
    "MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=$HOME/.claude pnpm tauri dev",
    "cd x && pnpm tauri dev",
    "nohup pnpm tauri dev &",
    "MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=/tmp/cc pnpm tauri dev",
    "MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=/tmp/cc MARSHELL_CLAUDE_SETTINGS= pnpm tauri dev",
    'MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=/tmp/cc MARSHELL_CLAUDE_SETTINGS="" pnpm tauri dev',
    "MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=/tmp/cc MARSHELL_CLAUDE_SETTINGS=~/.claude/settings.json pnpm tauri dev",
    "MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=/tmp/cc MARSHELL_CLAUDE_SETTINGS=$HOME/.claude.json pnpm tauri dev",
    "MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=/tmp/cc MARSHELL_CLAUDE_SETTINGS=~/.claude/x/settings.local.json pnpm tauri dev",
  ];
  for (const c of bad) test(`Bash: ${c}`, () => denied(shell(c), R.S6));
  const badPs = [
    'pnpm tauri dev',
    '$env:MARSHELL_HOME="C:\\s\\mh"; pnpm tauri dev',
    '$env:MARSHELL_HOME="C:\\s\\mh"; $env:CLAUDE_CONFIG_DIR="$env:USERPROFILE\\.claude"; pnpm tauri dev',
    '$env:MARSHELL_HOME="$env:USERPROFILE\\.marshell"; $env:CLAUDE_CONFIG_DIR="C:\\s\\cc"; pnpm tauri dev',
    '$env:MARSHELL_HOME="C:\\s\\mh"; $env:CLAUDE_CONFIG_DIR="C:\\s\\cc"; pnpm tauri dev',
    '$env:MARSHELL_HOME="C:\\s\\mh"; $env:CLAUDE_CONFIG_DIR="C:\\s\\cc"; $env:MARSHELL_CLAUDE_SETTINGS="$env:USERPROFILE\\.claude\\settings.json"; pnpm tauri dev',
  ];
  for (const c of badPs) test(`PowerShell: ${c}`, () => denied(ps(c), R.S6));

  const SCRATCH3 = "MARSHELL_HOME=/tmp/mh CLAUDE_CONFIG_DIR=/tmp/cc MARSHELL_CLAUDE_SETTINGS=/tmp/p/settings.local.json";
  test("Bash: all three set to scratch folders", () =>
    none(shell(`${SCRATCH3} WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 pnpm tauri dev`)));
  test("Bash: env prefix with all three set", () => none(shell(`env ${SCRATCH3} pnpm tauri dev`)));
  test("Bash: nohup with all three set", () => none(shell(`${SCRATCH3} nohup pnpm tauri dev &`)));
  test("PowerShell: all three set to scratch folders", () =>
    none(
      ps(
        '$env:MARSHELL_HOME="C:\\s\\mh"; $env:CLAUDE_CONFIG_DIR="C:\\s\\cc"; $env:MARSHELL_CLAUDE_SETTINGS="C:\\s\\p\\settings.local.json"; pnpm tauri dev',
      ),
    ));
  test("Bash: 8.3 short name of the home folder is the real home", { skip: NO_SHORT }, () =>
    denied(
      shell(
        `MARSHELL_HOME=${fwd(SHORT_HOME)}/.marshell CLAUDE_CONFIG_DIR=/c/s/cc MARSHELL_CLAUDE_SETTINGS=/c/s/p/settings.local.json pnpm tauri dev`,
      ),
      R.S6,
    ));
  test('no decision: grep -n "tauri dev" docs/PLAN.md', () => none(shell('grep -n "tauri dev" docs/PLAN.md')));
  test("no decision: pnpm tauri build", () => none(shell("pnpm tauri build --no-bundle")));
});

// ---- S7 ----------------------------------------------------------------------------------------------------------
describe("S7: irreversible git operations ask", () => {
  const bad = [
    "git push --force origin main",
    "git push origin main -f",
    "git push --force-with-lease",
    "git reset --hard HEAD~1",
    "git clean -fd",
    "git clean -f",
    "git branch -D marshell-canary-none",
    "git -C C:\\dev\\x reset --hard",
    "git branch --delete --force foo",
    "git branch -d -f foo",
    "git branch -df foo",
    "git push origin --delete foo",
    "git push -d origin foo",
    "git push origin :foo",
  ];
  for (const c of bad) test(`Bash: ${c}`, () => asked(shell(c), R.S7));
  test("PowerShell: git push --force", () => asked(ps("git push --force"), R.S7));
  test("PowerShell: git branch -D x", () => asked(ps("git branch -D x"), R.S7));

  const ok = [
    "git push origin main",
    "git push -u origin x",
    "git branch -d x",
    "git branch --delete foo",
    "git push origin HEAD:refs/heads/foo",
    "git reset --soft HEAD~1",
    "git clean -n",
    'git commit -m "docs: git push --force and git reset --hard ask first"',
  ];
  for (const c of ok) test(`no decision: ${c}`, () => none(shell(c, { cwd: FAKE_HOME })));

  test("deny wins over ask", () => denied(shell("git push --force && taskkill /IM x.exe"), R.S1));
});

// ---- S8 ----------------------------------------------------------------------------------------------------------
describe("S8: Rust commit needs cargo fmt", () => {
  const UNFORMATTED = "pub fn   a( ) { }\n";
  const FORMATTED = "pub fn a() {}\n";
  const CARGO_TOML = '[package]\nname = "fx"\nversion = "0.0.0"\nedition = "2021"\n';

  function rustRepo(source) {
    const dir = mkRepo();
    writeAndStage(dir, "Cargo.toml", CARGO_TOML);
    writeAndStage(dir, "src/lib.rs", source);
    return dir;
  }
  const opts = { realHome: true };
  const cargo = { skip: !HAS_CARGO && "cargo not installed" };

  // The guard fails open when cargo fmt takes over 10 s. On a cold CI runner the first two runs in a new repo took
  // 11.7 s and 7 s, and later ones 0.4 s, so warm it up untimed before the tests that time it.
  before(() => {
    if (!HAS_CARGO) return;
    for (let i = 0; i < 2; i++) {
      spawnSync("cargo", ["fmt", "--all", "--check"], { cwd: rustRepo(FORMATTED), encoding: "utf8", windowsHide: true });
    }
  });

  test("unformatted .rs staged, cwd is the repo", cargo, () => {
    const dir = rustRepo(UNFORMATTED);
    denied(shell('git commit -m "x"', { cwd: dir, ...opts }), R.S8);
  });
  test("unformatted .rs staged, git -C <dir>", cargo, () => {
    const dir = rustRepo(UNFORMATTED);
    denied(shell(`git -C "${dir}" commit -m "x"`, { cwd: os.tmpdir(), ...opts }), R.S8);
  });
  test("unformatted .rs staged, cd <dir> && git commit", cargo, () => {
    const dir = rustRepo(UNFORMATTED);
    denied(shell(`cd "${fwd(dir)}" && git commit -m "x"`, { cwd: os.tmpdir(), ...opts }), R.S8);
  });
  test("unformatted .rs staged, PowerShell Set-Location", cargo, () => {
    const dir = rustRepo(UNFORMATTED);
    denied(ps(`Set-Location "${dir}"; git commit -m "x"`, { cwd: os.tmpdir(), ...opts }), R.S8);
  });
  test("formatted .rs staged gives no decision", cargo, () => {
    const dir = rustRepo(FORMATTED);
    none(shell('git commit -m "x"', { cwd: dir, ...opts }));
  });
  test("only .ts staged gives no decision", () => {
    const dir = mkRepo();
    writeAndStage(dir, "src/a.ts", "export const a = 1;\n");
    fs.writeFileSync(path.join(dir, "src", "bad.rs"), UNFORMATTED); // unstaged Rust is not checked
    none(shell('git commit -m "feat: a"', { cwd: dir, ...opts }));
  });
  test("unresolvable target (missing folder) fails open", () => {
    none(shell(`git -C "${path.join(os.tmpdir(), "guards-no-such-folder")}" commit -m x`, { cwd: os.tmpdir(), ...opts }));
  });
  test("cwd outside any repo fails open", () => {
    none(shell('git commit -m "x"', { cwd: FAKE_HOME, ...opts }));
  });
  test("a repo that is not this one is not checked", () => {
    const dir = mkRepo({ marshell: false });
    writeAndStage(dir, "Cargo.toml", CARGO_TOML);
    writeAndStage(dir, "src/lib.rs", UNFORMATTED);
    none(shell('git commit -m "x"', { cwd: dir, ...opts }));
  });
  test("missing git fails open", () => {
    const dir = rustRepo(UNFORMATTED);
    none(shell('git commit -m "x"', { cwd: dir, env: envWithPath("") }));
  });
  test("missing cargo fails open", () => {
    const dir = rustRepo(UNFORMATTED);
    const gitDir = (process.env.PATH ?? process.env.Path ?? "")
      .split(path.delimiter)
      .find((d) => d && fs.existsSync(path.join(d, WIN ? "git.exe" : "git")));
    if (!gitDir || fs.existsSync(path.join(gitDir, WIN ? "cargo.exe" : "cargo"))) return; // can't isolate git from cargo here
    none(shell('git commit -m "x"', { cwd: dir, env: { ...envWithPath(gitDir), ...FAKE_HOME_ENV } }));
  });
});

// ---- Review Focus 1: mentioning a command is not running it ------------------------------------------------------
describe("mention-only commands give no decision", () => {
  const dir = mkRepo();
  writeAndStage(dir, "src/a.ts", "export const a = 1;\n");

  test('grep -n "tauri dev" docs/PLAN.md', () => none(shell('grep -n "tauri dev" docs/PLAN.md', { cwd: dir })));
  test("a heredoc whose body holds git stash and taskkill /IM", () => {
    const c = [
      "codex exec - <<'EOF'",
      "Run git stash pop, then taskkill /IM claude.exe and rm -rf $HOME/x.",
      "Also read ~/.claude/.credentials.json and pnpm tauri dev.",
      "EOF",
    ].join("\n");
    none(shell(c, { cwd: dir }));
  });
  test("a heredoc followed by a real command still checks that command", () => {
    const c = ["cat <<'EOF' > notes.md", "git stash", "EOF", "git stash pop"].join("\n");
    denied(shell(c, { cwd: dir }), R.S3);
  });
  test("a PowerShell here-string whose body holds git stash", () => {
    const c = ["$p = @'", "git stash pop", "taskkill /IM x.exe", "'@", "codex exec $p"].join("\n");
    none(ps(c, { cwd: dir }));
  });
  test('echo "Ruling: taskkill /IM is denied" >> progress.md', () =>
    none(shell('echo "Ruling: taskkill /IM is denied" >> progress.md', { cwd: dir })));
  test('git commit -m "docs: explain why git stash pop is denied"', () =>
    none(shell('git commit -m "docs: explain why git stash pop is denied"', { cwd: dir, realHome: true })));
  test("Claude Code's commit form: a heredoc message with a double quote inside $(...)", () => {
    const c = [
      "git commit -m \"$(cat <<'EOF'",
      'fix: why git stash pop and taskkill /IM are denied, "quoted"',
      "EOF",
      ')"',
    ].join("\n");
    none(shell(c, { cwd: dir, realHome: true }));
  });
  test("continuation lines are joined", () => {
    denied(shell("git \\\nstash pop", { cwd: dir }), R.S3);
  });
});

// ---- file rules --------------------------------------------------------------------------------------------------
describe("file rules", () => {
  const mz = mkRepo();
  const other = mkRepo({ marshell: false });
  const at = (...p) => path.join(mz, ...p);

  describe("F1: src/generated", () => {
    test("Write, native separators", () => denied(edit(at("src", "generated", "x.ts"), { tool: "Write" }), R.F1));
    test("Edit, forward slashes", () => denied(edit(fwd(at("src", "generated", "protocol.ts"))), R.F1));
    test("MultiEdit, nested file", () => denied(edit(at("src", "generated", "a", "b.ts"), { tool: "MultiEdit" }), R.F1));
    test("NotebookEdit via notebook_path", () =>
      denied(edit(at("src", "generated", "n.ipynb"), { tool: "NotebookEdit", key: "notebook_path" }), R.F1));
    test("relative path resolved against cwd", () =>
      denied(edit("src\\generated\\x.ts", { tool: "Write", cwd: mz }), R.F1));
    test("a nested worktree with its own .git is its own checkout", () => {
      const wt = path.join(mz, ".claude", "worktrees", "x");
      for (const d of ["crates/protocol", "src-tauri", "src/generated"]) fs.mkdirSync(path.join(wt, d), { recursive: true });
      fs.writeFileSync(path.join(wt, ".git"), "gitdir: nowhere\n");
      denied(edit(fwd(path.join(wt, "src", "generated", "a.ts"))), R.F1);
      denied(edit(path.join(wt, "src", "generated", "a.ts")), R.F1);
    });
    test("src/lib/a.ts gives no decision", () => none(edit(at("src", "lib", "a.ts"))));
    test("src/generated-notes.ts gives no decision", () => none(edit(at("src", "generated-notes.ts"))));
    test("a different repo does not match", () => none(edit(path.join(other, "src", "generated", "x.ts"))));
    test("C:/dev/claude-setup/src/generated/x.ts gives no decision", () =>
      none(edit("C:/dev/claude-setup/src/generated/x.ts")));
  });

  describe("F2: owner files", () => {
    for (const f of ["SCOPE.md", "docs/BRIEF.md", "LICENSE", "THIRD_PARTY_NOTICES.md", "SECURITY.md"]) {
      test(`Write ${f}`, () => denied(edit(at(...f.split("/")), { tool: "Write" }), R.F2));
    }
    test("backslashes and forward slashes", () => {
      denied(edit(`${mz}\\docs\\BRIEF.md`), R.F2);
      denied(edit(`${fwd(mz)}/SCOPE.md`), R.F2);
    });
    test("drive letter and path in another case", { skip: !WIN && "Windows only" }, () => {
      const odd = mz.slice(0, 2).toLowerCase() + mz.slice(2).toUpperCase() + "\\SCOPE.md";
      denied(edit(odd), R.F2);
    });
    test("docs/PLAN.md and README.md give no decision", () => {
      none(edit(at("docs", "PLAN.md")));
      none(edit(at("README.md")));
    });
    test("a SCOPE.md in a subfolder gives no decision", () => none(edit(at("docs", "SCOPE.md"))));
    test("a different repo does not match", () => none(edit(path.join(other, "SCOPE.md"))));
  });

  describe("F3: security surface asks", () => {
    for (const f of [
      "src-tauri/capabilities/default.json",
      "src-tauri/tauri.conf.json",
      "src-tauri/tauri.windows.conf.json",
      "crates/core/src/server/guard.rs",
    ]) {
      test(`Edit ${f}`, () => asked(edit(at(...f.split("/"))), R.F3));
    }
    test("backslashes", () => asked(edit(`${mz}\\crates\\core\\src\\server\\guard.rs`, { tool: "MultiEdit" }), R.F3));
    test("other src-tauri files give no decision", () => none(edit(at("src-tauri", "src", "main.rs"))));
    test("other server files give no decision", () => none(edit(at("crates", "core", "src", "server", "mod.rs"))));
  });

  describe("F4: real CLI configuration and data", () => {
    const home = FAKE_HOME;
    for (const f of [
      path.join(home, ".codex", "config.toml"),
      path.join(home, ".marshell", "run", "endpoint.json"),
      path.join(home, ".claude", "agents", "a.md"),
      path.join(home, ".claude", "CLAUDE.md"),
      `${fwd(home)}/.claude/skills/x/SKILL.md`,
    ]) {
      test(`Write ${f}`, () => denied(edit(f, { tool: "Write" }), R.F4));
    }
    test("~/.claude.json is the real Claude user config", () => {
      denied(edit(path.join(home, ".claude.json"), { tool: "Write" }), R.F4);
      denied(edit("~/.claude.json"), R.F4);
    });
    test("a lookalike of .claude.json gives no decision", () => none(edit(path.join(home, ".claude.json-x"))));
    test("8.3 short name of the home folder is the real home", { skip: NO_SHORT }, () =>
      denied(edit(`${fwd(SHORT_HOME)}/.claude/agents/x.md`, { tool: "Write" }), R.F4));
    test("~ is expanded", () => denied(edit("~/.codex/config.toml"), R.F4));
    test("%USERPROFILE% is expanded", () => denied(edit("%USERPROFILE%\\.claude\\agents\\a.md"), R.F4));
    test("$HOME is expanded", () => denied(edit("$HOME/.marshell/x.json"), R.F4));
    test("$env:USERPROFILE is expanded", () => denied(edit("$env:USERPROFILE\\.codex\\auth.json"), R.F4));
    test("${HOME} is expanded", () => denied(edit("${HOME}/.codex/x"), R.F4));
    test("~/.claude/projects/*/memory/** gives no decision", () => {
      none(edit(path.join(home, ".claude", "projects", "C--dev-Airport", "memory", "x.md")));
      none(edit("~/.claude/projects/C--dev-Airport/memory/MEMORY.md"));
    });
    test("~/.claude/settings.json gives no decision", () => {
      none(edit(path.join(home, ".claude", "settings.json")));
      none(edit("~/.claude/settings.json"));
    });
    test("plan mode's ~/.claude/plans/*.md gives no decision", () => {
      none(edit(path.join(home, ".claude", "plans", "sunny-rolling-codd.md")));
      none(edit("~/.claude/plans/x.md", { tool: "Write" }));
    });
    test("deeper or non-markdown files under plans are still denied", () => {
      denied(edit(path.join(home, ".claude", "plans", "a", "b.md")), R.F4);
      denied(edit(path.join(home, ".claude", "plans", "x.json")), R.F4);
    });
    test("other projects folders are still denied", () =>
      denied(edit(path.join(home, ".claude", "projects", "p", "session.jsonl")), R.F4));
    test("a lookalike folder gives no decision", () => none(edit(path.join(home, ".claude-other", "a.md"))));
  });
});
