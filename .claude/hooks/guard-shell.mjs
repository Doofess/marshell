// PreToolUse guard for Bash and PowerShell: rules S1 to S8 of SCOPE.md and the live-check rules.
// A rule matches only where a command word starts a segment, so mentioning a command is never running it.
import fs from "node:fs";
import os from "node:os";
import { spawnSync } from "node:child_process";
import { P, ask, deny, findGitRoot, isMarshellRoot, key, runHook, toAbs } from "./lib.mjs";

const SHELL_TOOLS = new Set(["Bash", "PowerShell"]);

const S1 = "Stop processes by PID only, after listing the tree: real agent CLIs run on this machine (SCOPE.md).";
const S2 = "Don't delete a variable-built path recursively. Use a literal scratch path, and ask before cleaning scratch folders.";
const S3 = 'The stash is shared with other sessions. Use a WIP commit, or git stash push -u -m "<tag>" and apply by SHA.';
const S4 = "Credential files and the real Marshell token are off limits (SCOPE.md). A live check reads its scratch home's token.";
const S5 = "Don't name a test binary like a real CLI. Use a name like codex-marshell-test.exe.";
const S6 = "A live check runs with MARSHELL_HOME and CLAUDE_CONFIG_DIR set to scratch folders. See the marshell-live-check skill.";
const S7 = "This can't be undone. Confirm with the user first.";
const S8 = "Run cargo fmt --all before a Rust commit.";

// ---- reading the command -----------------------------------------------------------------------------------------

/** Joins continuation lines and drops heredoc bodies (bash) and here-strings (PowerShell). */
function stripBodies(command) {
  let s = command.replace(/\r\n?/g, "\n").replace(/\\\n/g, " ").replace(/`\n/g, " ");
  s = s.replace(/@(['"])[ \t]*\n[\s\S]*?\n[ \t]*\1@/g, '""');
  const out = [];
  const pending = [];
  for (const line of s.split("\n")) {
    if (pending.length) {
      if (line.trim() === pending[0]) pending.shift();
      continue;
    }
    out.push(line);
    const opener = /(?<!<)<<-?[ \t]*(['"]?)([A-Za-z_][\w.-]*)\1(?!<)/g;
    for (let m = opener.exec(line); m; m = opener.exec(line)) pending.push(m[2]);
  }
  return out.join("\n");
}

/**
 * Splits a command into segments at ; && || | & newline and parentheses. Quoted text stays inside its word, so a
 * quoted string never starts a command. Each segment is `{ sep, words: [{ t, q }] }`, `sep` being the separator before it.
 */
function tokenize(s) {
  const segs = [];
  let words = [];
  let sep = "";
  let cur = "";
  let has = false;
  let quoted = false;
  const endWord = () => {
    if (has) words.push({ t: cur, q: quoted });
    cur = "";
    has = false;
    quoted = false;
  };
  const endSeg = (next, soft = false) => {
    endWord();
    const had = words.length > 0;
    if (had) segs.push({ sep, words });
    words = [];
    if (had || !soft) sep = next;
  };
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === " " || c === "\t") endWord();
    else if (c === "\n") endSeg("\n");
    else if (c === ";") endSeg(";");
    else if (c === "|") {
      if (s[i + 1] === "|") {
        i++;
        endSeg("||");
      } else {
        if (s[i + 1] === "&") i++;
        endSeg("|");
      }
    } else if (c === "&") {
      if (s[i + 1] === "&") {
        i++;
        endSeg("&&");
      } else if ((has && /[<>]$/.test(cur)) || s[i + 1] === ">") {
        cur += c;
        has = true;
      } else if (has || words.length > 0) endSeg("&");
      // else: PowerShell's call operator at the start of a segment, dropped
    } else if (c === "(" || c === ")") endSeg(c, true);
    else if (c === "#" && !has) {
      const nl = s.indexOf("\n", i);
      i = nl === -1 ? s.length : nl - 1;
    } else if (c === "'") {
      const end = s.indexOf("'", i + 1);
      const stop = end === -1 ? s.length : end;
      cur += s.slice(i + 1, stop);
      has = quoted = true;
      i = stop;
    } else if (c === '"') {
      let j = i + 1;
      let buf = "";
      while (j < s.length && s[j] !== '"') {
        if ((s[j] === "\\" || s[j] === "`") && s[j + 1] === '"') {
          buf += '"';
          j += 2;
        } else buf += s[j++];
      }
      cur += buf;
      has = quoted = true;
      i = j;
    } else {
      cur += c;
      has = true;
    }
  }
  endSeg("");
  return segs;
}

const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/;

/** The command word of a segment (lower case, no folder, no .exe) and its arguments, skipping env, VAR=x and { prefixes. */
function commandOf(words) {
  let i = 0;
  while (i < words.length) {
    const { t, q } = words[i];
    if (!q && (t === "{" || t === "}" || t === "!")) i++;
    else if (ASSIGNMENT.test(t)) i++;
    else if (!q && t.toLowerCase() === "env") {
      i++;
      while (i < words.length && (words[i].t.startsWith("-") || ASSIGNMENT.test(words[i].t))) i++;
    } else break;
  }
  if (i >= words.length) return null;
  const name = words[i].t.replace(/\\/g, "/").split("/").pop().toLowerCase().replace(/\.exe$/, "");
  return { name, args: words.slice(i + 1).map((w) => w.t) };
}

/** git's subcommand and arguments, with its global options (-C dir, -c k=v, --no-pager ...) taken out. */
function parseGit(args) {
  const dirs = [];
  let i = 0;
  for (; i < args.length; i++) {
    const a = args[i];
    if (a === "-C") dirs.push(args[++i]);
    else if (a === "-c" || a === "--git-dir" || a === "--work-tree" || a === "--namespace") i++;
    else if (!a.startsWith("-")) break;
  }
  return { dirs: dirs.filter((d) => typeof d === "string"), sub: (args[i] ?? "").toLowerCase(), rest: args.slice(i + 1) };
}

/** The value of `--flag=value` or `-Flag:value`, else the word itself. */
const pathPart = (a) => (a.startsWith("-") ? a.replace(/^-+[^=:]*[=:]/, "") : a);
const lastSegment = (a) => key(pathPart(a)).split("/").pop();

// ---- rules -------------------------------------------------------------------------------------------------------

const RM_FLAGS = /^-[rRfiIvdx]*[rR][rRfiIvdx]*$|^--recursive$|^-r(e(c(u(r(s(e)?)?)?)?)?)?$/i;
const SENSITIVE_SUFFIXES = ["/.codex/auth.json", "/.marshell/run/endpoint.json"];

function isEnvFile(base) {
  if (base === ".env") return true;
  return base.startsWith(".env.") && !/^\.env\.(example|sample|template|dist)$/.test(base);
}

function touchesSensitiveFile(args) {
  return args.some((a) => {
    const k = "/" + key(pathPart(a));
    const base = k.split("/").pop();
    return base === ".credentials.json" || isEnvFile(base) || SENSITIVE_SUFFIXES.some((s) => k.endsWith(s));
  });
}

/** S6: `tauri dev` in a runner command. */
function isTauriDev(name, args) {
  if (!["pnpm", "npm", "npx", "yarn", "bun", "bunx", "pnpx", "cargo", "cargo-tauri", "tauri"].includes(name)) return false;
  const toks = [name, ...args].map((a) => a.toLowerCase());
  for (let i = 0; i < toks.length; i++) {
    if (toks[i] !== "tauri") continue;
    let j = i + 1;
    while (toks[j] === "--") j++;
    if (toks[j] === "dev") return true;
  }
  return name === "tauri" && toks[1] === "dev";
}

/** S6: whether the same command sets `name`, and whether any value it sets points into the real home folder. */
function envSetting(command, name) {
  const mention = new RegExp(`\\$\\{?env:${name}\\b|\\b${name}\\s*=`, "i");
  const assign = new RegExp(`(?:\\$\\{?env:)?\\b${name}\\}?\\s*=\\s*("([^"]*)"|'([^']*)'|[^\\s;&|)]+)`, "gi");
  const home = key(P.resolve(os.homedir()));
  let real = false;
  for (const m of command.matchAll(assign)) {
    const value = m[2] ?? m[3] ?? m[1];
    const v = key(toAbs(value));
    for (const dir of [".marshell", ".claude"]) {
      const base = `${home}/${dir}`;
      if (v === base || v.startsWith(`${base}/`)) real = true;
    }
  }
  return { set: mention.test(command), real };
}

/** S8: whether `target` is a checkout of this repo with staged Rust files that `cargo fmt --all --check` rejects. */
function rustNeedsFmt(target) {
  if (!fs.existsSync(target)) return false;
  const root = findGitRoot(target);
  if (!root || !isMarshellRoot(root)) return false;
  const staged = spawnSync("git", ["-C", target, "diff", "--cached", "--name-only"], {
    encoding: "utf8",
    timeout: 3000,
    windowsHide: true,
  });
  if (staged.error || staged.status !== 0) return false;
  if (!staged.stdout.split(/\r?\n/).some((f) => /\.rs$/i.test(f.trim()))) return false;
  const fmt = spawnSync("cargo", ["fmt", "--all", "--check"], { cwd: target, encoding: "utf8", timeout: 10000, windowsHide: true });
  if (fmt.error || fmt.status === null) return false;
  return fmt.status !== 0;
}

function evaluate(command, cwd) {
  const body = stripBodies(command);
  const segs = tokenize(body);
  let denial = null;
  let question = null;
  const found = (d) => {
    if (d.decision === "deny") denial ??= d;
    else question ??= d;
  };
  const commitTargets = [];
  let dir = cwd;
  let prev = null;

  for (const seg of segs) {
    const cmd = commandOf(seg.words);
    const before = prev;
    prev = cmd;
    if (!cmd) continue;
    const { name, args } = cmd;
    const lower = args.map((a) => a.toLowerCase());
    const plain = args.filter((a) => !a.startsWith("-"));

    // S1: killing by name
    if (name === "pkill" || name === "killall") found(deny(S1));
    else if (name === "taskkill" && args.some((a) => /^[/-]{1,2}im$/i.test(a))) found(deny(S1));
    else if (name === "stop-process") {
      if (lower.some((a) => /^-(name|processname)(:|$)/.test(a))) found(deny(S1));
      else if (seg.sep === "|" && before && ["get-process", "gps"].includes(before.name)) found(deny(S1));
    }

    // S2: recursive delete of a variable-built path
    if ((name === "rm" || name === "remove-item") && args.some((a) => RM_FLAGS.test(a)) && args.some((a) => /[$`%]/.test(a))) {
      found(deny(S2));
    }

    // S4: credential files and the real Marshell token
    if (touchesSensitiveFile(args)) found(deny(S4));

    // S5: a test binary named like a real CLI
    if (["cp", "copy", "copy-item", "mv", "move", "move-item", "rename-item"].includes(name)) {
      if (args.some((a) => ["claude.exe", "codex.exe"].includes(lastSegment(a)))) found(deny(S5));
    }

    // S6: tauri dev outside scratch folders
    if (isTauriDev(name, args)) {
      const home = envSetting(body, "MARSHELL_HOME");
      const config = envSetting(body, "CLAUDE_CONFIG_DIR");
      if (!home.set || !config.set || home.real || config.real) found(deny(S6));
    }

    // directory changes, for S8
    if (["cd", "chdir", "set-location", "sl", "pushd"].includes(name)) {
      const target = plain.find((a) => a.toLowerCase() !== "/d" && a !== "-");
      if (target && dir) dir = toAbs(target, dir);
    }

    if (name === "git") {
      const { dirs, sub, rest } = parseGit(args);
      const hasFlag = (re) => rest.some((a) => re.test(a));
      if (sub === "stash") {
        const op = rest.find((a) => !a.startsWith("-"))?.toLowerCase() ?? "";
        const withMessage = hasFlag(/^(-m.*|--message(=.*)?)$/);
        if (op === "pop" || op === "save" || ((op === "" || op === "push") && !withMessage)) found(deny(S3));
      } else if (sub === "push" && hasFlag(/^(--force(-with-lease(=.*)?)?|-[nvqud46]*f[nvqud46]*)$/)) found(ask(S7));
      else if (sub === "reset" && rest.includes("--hard")) found(ask(S7));
      else if (sub === "clean" && hasFlag(/^(--force|-[a-zA-Z]*f[a-zA-Z]*)$/)) found(ask(S7));
      else if (sub === "branch" && hasFlag(/^-[a-zA-Z]*D[a-zA-Z]*$/)) found(ask(S7));
      else if (sub === "commit") {
        let target = dir;
        for (const d of dirs) target = target ? toAbs(d, target) : toAbs(d);
        if (target) commitTargets.push(target);
      }
    }
  }

  // S8 last: it starts processes, so it runs only when nothing cheaper has already denied.
  if (!denial) {
    for (const target of commitTargets) {
      if (rustNeedsFmt(target)) {
        found(deny(S8));
        break;
      }
    }
  }
  return denial ?? question;
}

runHook((input) => {
  if (!SHELL_TOOLS.has(input.tool_name)) return null;
  const command = input.tool_input?.command;
  if (typeof command !== "string" || !command.trim()) return null;
  return evaluate(command, typeof input.cwd === "string" && input.cwd ? input.cwd : process.cwd());
});
