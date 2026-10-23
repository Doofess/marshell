// PreToolUse guard for Bash and PowerShell: rules S1 to S8 of SCOPE.md and the live-check rules.
// A rule matches only where a command word starts a segment, so mentioning a command is never running it.
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { ask, canon, deny, findGitRoot, homeKey, isMarshellRoot, key, runHook, toAbs } from "./lib.mjs";

const SHELL_TOOLS = new Set(["Bash", "PowerShell"]);

const S1 = "Stop processes by PID only, after listing the tree: real agent CLIs run on this machine (SCOPE.md).";
const S2 = "Don't delete a variable-built path recursively. Use a literal scratch path, and ask before cleaning scratch folders.";
const S3 = 'The stash is shared with other sessions. Use a WIP commit, or git stash push -u -m "<tag>" and apply by SHA.';
const S3D =
  "The stash is shared by every session and worktree, so clear or an unnamed drop can destroy another session's work. Drop a stash by its SHA, and only your own.";
const S4 = "Credential files and the real Marshell token are off limits (SCOPE.md). A live check reads its scratch home's token.";
const S5 = "Don't name a test binary like a real CLI. Use a name like codex-marshell-test.exe.";
const S6 =
  "A live check runs with MARSHELL_HOME, CLAUDE_CONFIG_DIR and MARSHELL_CLAUDE_SETTINGS set to scratch folders. See the marshell-live-check skill.";
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

/** Shell keywords and wrappers that come before the real command word: `do rm`, `then git`, `try { git`, `time git`. */
const KEYWORDS = new Set(["do", "then", "else", "elif", "if", "while", "until", "time", "nohup", "!", "{", "}", "try", "catch", "finally"]);

/**
 * The command word of a segment (lower case, no folder, no .exe) and its arguments, skipping shell keywords, env and
 * VAR=x prefixes.
 */
function commandOf(words) {
  let i = 0;
  while (i < words.length) {
    const { t, q } = words[i];
    const low = t.toLowerCase();
    if (!q && KEYWORDS.has(low)) {
      i++;
      if (low === "time") while (i < words.length && words[i].t.startsWith("-")) i++;
      if (low === "catch") while (i < words.length && words[i].t.startsWith("[")) i++;
    } else if (ASSIGNMENT.test(t)) i++;
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

const REMOVE_ITEM = new Set(["rm", "remove-item", "ri", "rmdir", "rd", "del", "erase"]);

/** Whether a pipeline starts from `Get-Process` without -Id, so what it feeds on is chosen by name or is everything. */
function listsProcessesByName(cmd) {
  if (!cmd || !["get-process", "gps"].includes(cmd.name)) return false;
  return !cmd.args.some((a) => /^-id(:|$)/i.test(a));
}

const RM_FLAGS =/^-[rRfiIvdx]*[rR][rRfiIvdx]*$|^--recursive$|^-r(e(c(u(r(s(e)?)?)?)?)?)?$/i;
const SENSITIVE_SUFFIXES = ["/.codex/auth.json", "/.marshell/run/endpoint.json"];

function isEnvFile(base) {
  if (base === ".env") return true;
  return base.startsWith(".env.") && !/\.(example|sample|template|dist)$/.test(base);
}

// S4 looks only at file operands of commands that read or copy. These don't read what they are given:
const NON_READERS = new Set(["echo", "printf", "write-host", "write-output", "write-error", "write-warning", "write-verbose", "write-information"]);
const GIT_NON_READERS = new Set(["log", "commit", "status", "check-ignore", "ls-files", "shortlog", "rev-list", "whatchanged"]);
// These take a search pattern as the first positional argument; the rest are files.
const PATTERN_CMDS = new Set(["grep", "egrep", "fgrep", "rg", "ag", "ack", "findstr", "select-string", "sls"]);
const PATTERN_FLAG = /^(?:-e|--regexp|-pat(?:t(?:e(?:r(?:n)?)?)?)?)(?:[:=](.*))?$/i;
const FILE_FLAG = /^(?:-f|--file|-path|-literalpath)(?:[:=](.*))?$/i;
// Flags whose value is the next word (grep and rg are case sensitive; Select-String's are not, and are matched lower case).
const VALUE_FLAGS = new Set(["-A", "-B", "-C", "-m", "-d", "-D", "-g", "-t", "-T", "-j", "-M"]);
const LONG_VALUE_FLAGS = new Set([
  "--glob", "--type", "--type-not", "--include", "--exclude", "--exclude-dir", "--max-count", "--max-depth", "--threads",
  "--context", "--after-context", "--before-context", "--replace", "--max-columns", "--encoding", "--colors",
  "-context", "-encoding", "-include", "-exclude",
]);

/** The words of a pattern command that are files: its flags, and the pattern unless -e/-f/-Pattern gave one, are left out. */
function patternCmdFiles(name, args) {
  const files = [];
  let patternGiven = false;
  let rest = false;
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (!rest && a === "--") {
      rest = true;
      continue;
    }
    const isFlag = !rest && (name === "findstr" ? /^\/[a-z](?::|$)/i.test(a) : a.length > 1 && a.startsWith("-"));
    if (!isFlag) {
      if (patternGiven) files.push(a);
      else patternGiven = true;
      continue;
    }
    if (name === "findstr") {
      if (/^\/[cg]:/i.test(a)) patternGiven = true;
      continue;
    }
    let m;
    if ((m = PATTERN_FLAG.exec(a))) {
      patternGiven = true;
      if (m[1] === undefined) i++;
    } else if ((m = FILE_FLAG.exec(a))) {
      if (/^(-f|--file)/i.test(a)) patternGiven = true;
      if (m[1] !== undefined) files.push(m[1]);
      else if (i + 1 < args.length) files.push(args[++i]);
    } else if (VALUE_FLAGS.has(a) || LONG_VALUE_FLAGS.has(a.toLowerCase()) || (name === "rg" && a === "-r")) i++;
    // other flags (-rn, -i, --include=x, -SimpleMatch ...) carry no file
  }
  return files;
}

/** The operands of a command that it reads or copies, for S4. */
function readOperands(name, args) {
  if (NON_READERS.has(name)) return [];
  if (name === "git") return GIT_NON_READERS.has(parseGit(args).sub) ? [] : args;
  if (PATTERN_CMDS.has(name)) return patternCmdFiles(name, args);
  return args;
}

function touchesSensitiveFile(name, args) {
  return readOperands(name, args).some((a) => {
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

/**
 * S6: whether the same command sets `name` to something non-empty, and whether any value it sets points into the real
 * home's .marshell or .claude folder, or at ~/.claude.json.
 */
function envSetting(command, name) {
  // `$env:NAME = value` (PowerShell) or `NAME=value` straight after the equals sign (so `NAME= cmd` is empty).
  const assign = new RegExp(`(?:\\$\\{?env:${name}\\}?\\s*=\\s*|(?<![\\w$:])${name}=)("([^"]*)"|'([^']*)'|[^\\s;&|)]*)`, "gi");
  const home = homeKey();
  let set = false;
  let real = false;
  for (const m of command.matchAll(assign)) {
    const value = m[2] ?? m[3] ?? m[1];
    if (value.trim()) set = true;
    const v = key(canon(toAbs(value)));
    if (v === `${home}/.claude.json`) real = true;
    for (const dir of [".marshell", ".claude"]) {
      const base = `${home}/${dir}`;
      if (v === base || v.startsWith(`${base}/`)) real = true;
    }
  }
  return { set, real };
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
  let head = null; // the first command of the current pipeline

  for (const seg of segs) {
    const cmd = commandOf(seg.words);
    // A pipeline continues across | and across the parentheses of a script block or `(...).Method()`.
    if (seg.sep !== "|" && seg.sep !== "(" && seg.sep !== ")") head = cmd;
    else if (seg.sep === "(" && cmd && ["get-process", "gps"].includes(cmd.name)) head = cmd;
    if (!cmd) continue;
    const { name, args } = cmd;
    const lower = args.map((a) => a.toLowerCase());
    const plain = args.filter((a) => !a.startsWith("-"));

    // S1: killing by name
    if (name === "pkill" || name === "killall") found(deny(S1));
    else if (name === "taskkill" && args.some((a) => /^[/-]{1,2}im$/i.test(a))) found(deny(S1));
    else if (["stop-process", "spps", "kill"].includes(name)) {
      // `kill -n 9` is bash's signal flag, so for `kill` the name flag needs two letters.
      const byName = name === "kill" ? /^-(na(m(e)?)?|processname)(:|$)/i : /^-(n(a(m(e)?)?)?|processname)(:|$)/i;
      if (args.some((a) => byName.test(a))) found(deny(S1));
      else if (seg.sep === "|" && listsProcessesByName(head)) found(deny(S1));
    } else if (listsProcessesByName(head) && head !== cmd) {
      // (Get-Process x).Kill() and Get-Process x | ForEach-Object { $_.Kill() }
      const kills = name.startsWith(".kill") || args.some((a) => /\.kill\b/i.test(a));
      if (kills && (seg.sep === "|" || seg.sep === ")")) found(deny(S1));
    }

    // S2: recursive delete of a variable-built path
    if (REMOVE_ITEM.has(name) && args.some((a) => RM_FLAGS.test(a)) && args.some((a) => /[$`%]/.test(a))) {
      found(deny(S2));
    }

    // S4: credential files and the real Marshell token
    if (touchesSensitiveFile(name, args)) found(deny(S4));

    // S5: a test binary named like a real CLI
    if (["cp", "copy", "copy-item", "mv", "move", "move-item", "rename-item"].includes(name)) {
      if (args.some((a) => ["claude.exe", "codex.exe"].includes(lastSegment(a)))) found(deny(S5));
    }

    // S6: tauri dev outside scratch folders
    if (isTauriDev(name, args)) {
      const home = envSetting(body, "MARSHELL_HOME");
      const config = envSetting(body, "CLAUDE_CONFIG_DIR");
      const settings = envSetting(body, "MARSHELL_CLAUDE_SETTINGS");
      const all = [home, config, settings];
      if (all.some((e) => !e.set || e.real)) found(deny(S6));
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
        const opIdx = rest.findIndex((a) => !a.startsWith("-"));
        const op = opIdx < 0 ? "" : rest[opIdx].toLowerCase();
        const withMessage = hasFlag(/^(-m.*|--message(=.*)?)$/);
        if (op === "clear" || (op === "drop" && !rest.slice(opIdx + 1).some((a) => !a.startsWith("-")))) found(deny(S3D));
        else if (op === "pop" || op === "save" || ((op === "" || op === "push") && !withMessage)) found(deny(S3));
      } else if (sub === "push" && hasFlag(/^(--force(-with-lease(=.*)?)?|-[nvqud46]*f[nvqud46]*)$/)) found(ask(S7));
      else if (sub === "push" && (hasFlag(/^(--delete|-[nvqfu46]*d[nvqfu46]*)$/) || rest.some((a) => /^\+?:./.test(a)))) found(ask(S7));
      else if (sub === "reset" && rest.includes("--hard")) found(ask(S7));
      else if (sub === "clean" && hasFlag(/^(--force|-[a-zA-Z]*f[a-zA-Z]*)$/)) found(ask(S7));
      else if (sub === "branch" && (hasFlag(/^-[a-zA-Z]*D[a-zA-Z]*$/) || (hasFlag(/^(--delete|-[a-zA-Z]*d[a-zA-Z]*)$/) && hasFlag(/^(--force|-[a-zA-Z]*f[a-zA-Z]*)$/)))) {
        found(ask(S7));
      }
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
