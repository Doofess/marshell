# Claude Code project setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give this repo a project-scoped Claude Code setup. Hooks enforce `SCOPE.md` and the live-check rules. A webview MCP drives live checks. Project skills, an agent and path-scoped rules carry the repo's workflow. Tools Marshell never uses are switched off. Nothing changes outside this project.

**Architecture:**
- Everything lives in tracked files in the repo:
  - `.claude/settings.json`, `.mcp.json` and `.worktreeinclude`;
  - `.claude/hooks/*.mjs` (Node, with `node --test` tests);
  - `.claude/rules/*.md`, `.claude/skills/marshell-*/` and `.claude/agents/marshell-reviewer.md`.
- Every worktree cut from `main` after the merge carries the setup.
- The user's files are never edited: `~/.claude/settings.json`, `~/.claude.json`, `~/.claude/CLAUDE.md`, `~/.claude/agents/`, `~/.claude/skills/` and `C:/dev/claude-setup`.
- The one exception: `claude plugin install good-css@good-css --scope project` puts good-css into the plugin cache and `~/.claude/plugins/installed_plugins.json`. It is *enabled* only by this repo's settings, and other projects don't load it.

**Tech Stack:** Claude Code project settings, hooks, skills, agents and rules; Node 26 (`node --test`); `@playwright/mcp@0.0.83`.

**Spec:**
- The user's decisions of 2026-10-23, below.
- `docs/research/2026-10-23-claude-code-setup.md` (Task 1 writes it): the doc facts, with links, and the token inventory.

## Decisions (2026-10-23)

1. **Guards.** Hard rules are denied, with the reason shown to Claude. Edits to protected product files and irreversible git operations ask the user.
2. **Webview.** A project `.mcp.json` server named `webview` attaches Playwright to the live-check webview at `http://127.0.0.1:9333`. The global Playwright plugin stays for Storybook and the a11y matrix.
3. **Removed for this project:**
   - the claude.ai connectors Supabase, Resend and Cloudflare Developer Platform;
   - the synced `engineering` and `design` plugins;
   - `pyright-lsp`;
   - the `underwater-photoshop` and `dataviz` skills.
4. **Added:**
   - project skills `marshell-live-check`, `marshell-codex-task` and `marshell-ship`;
   - the `marshell-reviewer` agent (Opus);
   - path-scoped rules and a SessionStart resume hook;
   - the good-css plugin at project scope.

## Global Constraints

- **Project scope only.** No edit to `~/.claude/settings.json`, `~/.claude.json`, `~/.codex/` or `C:/dev/claude-setup`. A check that runs `claude` uses this repo or a scratch folder.
- **Owner files and binding docs stay unchanged:** `SCOPE.md`, `docs/BRIEF.md`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `SECURITY.md` and `AGENTS.md`. `CLAUDE.md` gains only pointers to the new pieces. Product code is not touched.
- **Hooks are Node `.mjs` with no dependencies.**
  - Each is called in shell form: `node "${CLAUDE_PROJECT_DIR}/.claude/hooks/<name>.mjs"`.
  - Each reads the hook JSON on stdin, answers in under 2 s (the commit format check under 10 s), and fails open. A crash or bad input exits 0 with no decision, so a bug never blocks work.
  - A deny uses `hookSpecificOutput.permissionDecision: "deny"` with a one-sentence reason that names the rule and the right way.
- **A skill or agent name never matches a user-level one.** Use the `marshell-` prefix.
- **Pinned versions.** `@playwright/mcp@0.0.83` was published 2026-09-28, more than two weeks ago. No `@latest`.
- **Copy:**
  - Every deny reason is written in this plan's tables and used verbatim.
  - The commit format follows the repo's `type(scope): sentence`. Commits by Claude end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **False positives that block real work.** Rules match only in command position, so mentioning a command is never running it:
   - `git stash list` and `git stash apply <sha>`;
   - `rm -rf` with a literal scratch path;
   - reading the *scratch* home's `run/endpoint.json`;
   - `taskkill /PID`;
   - a commit with no Rust files staged;
   - `grep -n "tauri dev" docs/PLAN.md`;
   - a heredoc whose body contains `git stash`, such as a Codex prompt;
   - `echo "Ruling: taskkill /IM is denied" >> progress.md`;
   - `git commit -m "docs: explain why git stash pop is denied"`.
   A guard that blocks these costs more than it saves.
2. **Guards fail open.** Empty stdin, non-JSON input, missing `tool_input`, a missing `git` or `cargo`, and a path outside any git checkout must all exit 0 with no output.
3. **Paths:**
   - Windows paths with backslashes, forward slashes, a drive letter in either case, `~`, `%USERPROFILE%`, `$HOME` and `$env:USERPROFILE`.
   - A file in a *different* repo must not match this repo's protected paths.
   - Worktrees: `.claude/worktrees/<x>/src/generated/a.ts` is generated code too.
4. **Project-only.** The SHA-256 hashes of the user's own files (Task 1 Step 1) are unchanged at the end of every task, apart from the good-css plugin cache.
5. **The resume hook stays small,** at most 15 lines, and silent when there's no ledger. It must not read whole ledgers into context.

---

### Task 1: Settings, MCP and the research note

**Files:**
- Create:
  - `.claude/settings.json`
  - `.mcp.json`
  - `.worktreeinclude`
  - `docs/research/2026-10-23-claude-code-setup.md`
- Modify: `.gitignore`, so that `.claude/settings.local.json` and `.playwright-mcp/` stay ignored. The second is already ignored; check the first.

**Produces:** the settings file that Tasks 2–3 add `hooks` to.

- [ ] **Step 1: Measure the baseline.** Every run is in this worktree, which has none of the new files yet.
  - **Tokens.** Run `claude -p "Reply with only: ok" --output-format json`. Record `usage.input_tokens + cache_creation_input_tokens + cache_read_input_tokens`.
  - **Inventory.** `claude mcp list` and `claude plugin list` give the deterministic inventory. `claude -p "List every skill name available to you, comma-separated, nothing else"` gives the skills. Save all three in the research note.
  - **User-file hashes.** Take SHA-256 hashes of:
    - `~/.claude/settings.json`, parsed with `advisorModel` removed, since the 5b run switches it;
    - `~/.claude/CLAUDE.md`;
    - every file under `~/.claude/agents/` and `~/.claude/skills/`;
    - `~/.claude.json`'s `projects` entries for paths outside this repo.
- [ ] **Step 2: Write `.claude/settings.json`.**
  ```json
  {
    "$schema": "https://json.schemastore.org/claude-code-settings.json",
    "enabledPlugins": {
      "pyright-lsp@claude-plugins-official": false,
      "engineering@synced": false,
      "design@synced": false,
      "good-css@good-css": true
    },
    "extraKnownMarketplaces": { "good-css": { "source": { "source": "github", "repo": "vojtaholik/good-css" } } },
    "deniedMcpServers": [
      { "serverName": "claude.ai Supabase" },
      { "serverName": "claude.ai Resend" },
      { "serverName": "claude.ai Cloudflare Developer Platform" }
    ],
    "skillOverrides": { "underwater-photoshop": "off", "dataviz": "off" },
    "enabledMcpjsonServers": ["webview"],
    "permissions": {
      "deny": [
        "Read(~/.claude/.credentials.json)",
        "Read(~/.codex/auth.json)",
        "Read(~/.marshell/run/endpoint.json)",
        "Read(**/.env)",
        "Read(**/.env.*)"
      ]
    }
  }
  ```
  Notes on these values:
  - **Connector names.** If `claude mcp list` shows a different display name for a connector, use that name and record it in the note.
  - **`skillOverrides`.** `"off"` is a documented value (skills docs, "Override skill visibility from settings"). `dataviz` is a bundled skill, which matches by name. `underwater-photoshop` is a user skill. Plugin skills ignore `skillOverrides`, which is why `engineering` and `design` are switched off through `enabledPlugins`.
- [ ] **Step 3: Install good-css at project scope.**
  - Run `claude plugin install good-css@good-css --scope project`.
  - Confirm that `git diff` shows only the `enabledPlugins` entry in the project file.
  - Confirm that `~/.claude/settings.json` has the same contents as before (byte-compare against a copy taken first).
- [ ] **Step 4: Write `.mcp.json`.**
  ```json
  { "mcpServers": { "webview": {
      "command": "cmd",
      "args": ["/c", "npx", "-y", "@playwright/mcp@0.0.83", "--cdp-endpoint", "http://127.0.0.1:9333",
               "--output-dir", "${CLAUDE_PROJECT_DIR:-.}/.playwright-mcp"] } } }
  ```
- [ ] **Step 5: Write `.worktreeinclude`.** It holds one line, `.claude/settings.local.json`, so per-person approvals reach new worktrees.
- [ ] **Step 6: Verify.**
  - Run the Step 1 commands again in this worktree.
  - **Expected:**
    - no `underwater-photoshop`, `dataviz`, `engineering:*` or `design:*` skill;
    - no Supabase, Resend or Cloudflare server in `claude mcp list`;
    - `webview` is listed;
    - the token count is lower.
  - **good-css.** `-p` may skip project plugin loading, so a missing good-css under `-p` proves nothing. `claude plugin list` must show it enabled at project scope.
  - **User files.** Every Step 1 hash is unchanged, apart from the plugin cache and `installed_plugins.json` entry for good-css.
  - **The user's look.** Ask the user for one interactive `/mcp` and `/plugin` look in a session started in this worktree, and record what they see.
  - Record all the numbers in the note.
- [ ] **Step 7: Write the research note.**
  - The doc facts this design rests on, each with its URL:
    - scopes and merging;
    - `@synced`;
    - `deniedMcpServers` merging;
    - hook deny under `bypassPermissions`;
    - worktree loading, including the Windows `settings.local.json` exception and the possible double `CLAUDE.md`;
    - skill-name precedence.
  - The before and after inventory.
  - One finding for the user's global setup, for the user to act on outside this repo: the Adobe connector's "off by default" can't be undone per project with `/mcp` if it's off through `deniedMcpServers`.
- [ ] **Step 8: Commit** as `chore(claude): add project Claude Code settings that switch off unused tools and add the webview MCP`.

### Task 2: Guard hooks

**Files:**
- Create:
  - `.claude/hooks/lib.mjs` (stdin JSON, `deny(reason)`, `ask(reason)`, git-root lookup, path normalizing)
  - `.claude/hooks/guard-shell.mjs` (matcher `Bash|PowerShell`)
  - `.claude/hooks/guard-files.mjs` (matcher `Edit|Write|MultiEdit|NotebookEdit`)
  - `.claude/hooks/guards.test.mjs`
- Modify: `.claude/settings.json` (the `hooks.PreToolUse` entries), `package.json` (script `"test:claude": "node --test .claude/hooks/"`) and `.github/workflows/ci.yml` (a `pnpm test:claude` step after `pnpm test`).

**Shell rules.**
- Before matching, the hook:
  - joins continuation lines (`\` and PowerShell's backtick);
  - removes heredoc bodies (bash `<<` heredocs and PowerShell here-strings);
  - removes quoted strings, keeping only the paths that S2, S4 and S5 inspect: arguments of the matched command itself.
- It then splits the command on `;`, `&&`, `||`, `|`, a newline and `(`.
- A rule matches only when its command word starts a segment. A leading `env VAR=…`, `VAR=… ` or `& ` prefix is skipped.
- Matching is case-insensitive.

| Id | Matches | Decision | Reason (verbatim) |
|---|---|---|---|
| S1 | `taskkill` with `/im`; `stop-process` with `-name` or `-processname`; `pkill`; `killall`; `get-process … \| stop-process` | deny | `Stop processes by PID only, after listing the tree: real agent CLIs run on this machine (SCOPE.md).` |
| S2 | `rm` with `-r`/`-rf`/`-fr`, or `remove-item` with `-recurse`, where a path argument contains `$`, a backtick or `%` | deny | `Don't delete a variable-built path recursively. Use a literal scratch path, and ask before cleaning scratch folders.` |
| S3 | `git stash` alone, `git stash pop`, `git stash save`, or `git stash push` without `-m` | deny | `The stash is shared with other sessions. Use a WIP commit, or git stash push -u -m "<tag>" and apply by SHA.` |
| S4 | `.credentials.json`; `.codex/auth.json` or `.codex\auth.json`; `.marshell/run/endpoint.json` or `.marshell\run\endpoint.json`; an `.env` file as an argument | deny | `Credential files and the real Marshell token are off limits (SCOPE.md). A live check reads its scratch home's token.` |
| S5 | `cp`, `copy`, `copy-item`, `mv`, `move`, `move-item` or `rename-item` with an argument whose file name is `claude.exe` or `codex.exe` | deny | `Don't name a test binary like a real CLI. Use a name like codex-marshell-test.exe.` |
| S6 | `tauri dev` without `MARSHELL_HOME=` (or `$env:MARSHELL_HOME`) and `CLAUDE_CONFIG_DIR=` (or `$env:CLAUDE_CONFIG_DIR`) in the same command, or with either one pointing at `.marshell` or `.claude` in the home folder | deny | `A live check runs with MARSHELL_HOME and CLAUDE_CONFIG_DIR set to scratch folders. See the marshell-live-check skill.` |
| S7 | `git push` with `--force`, `-f` or `--force-with-lease`; `git reset --hard`; `git clean -f…`; `git branch -D` | ask | `This can't be undone. Confirm with the user first.` |
| S8 | `git commit` while staged files include `*.rs`. The target checkout comes from `git -C <dir>`, else the last `cd <dir>`/`Set-Location <dir>` before it in the same command, else the hook's `cwd`. Run `cargo fmt --all --check` there; a non-zero exit denies. An unresolvable target, or no `cargo`, fails open | deny | `Run cargo fmt --all before a Rust commit.` |

**File rules.** The hook reads `tool_input.file_path`, or `notebook_path` for NotebookEdit. The path is made relative to the git root found by walking up from the file. A root counts as a checkout of this repo only if it contains `crates/protocol` and `src-tauri`.

| Id | Path (relative to its checkout) | Decision | Reason (verbatim) |
|---|---|---|---|
| F1 | `src/generated/**` | deny | `src/generated is written by ts-rs. Change crates/protocol and run cargo test -p marshell-protocol.` |
| F2 | `SCOPE.md`, `docs/BRIEF.md`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `SECURITY.md` | deny | `This is an owner file. Propose the change to the user instead.` |
| F3 | `src-tauri/capabilities/**`, `src-tauri/tauri.conf.json`, `crates/core/src/server/guard.rs` | ask | `Capabilities, the CSP and the API guard change only with the user's word (SCOPE.md).` |
| F4 | absolute paths under `~/.codex/`, `~/.marshell/`, or `~/.claude/`, except `~/.claude/projects/*/memory/**` and `~/.claude/settings.json`. The latter is exempt because the user's global rules have Claude switch `advisorModel` there | deny | `Real CLI configuration and Marshell data are off limits while developing (SCOPE.md).` |

- [ ] **Step 1: Write the failing tests** in `guards.test.mjs`, using the Write tool (never a Bash heredoc, which the finished guard would inspect). Each case runs the hook as a child process with a JSON stdin and asserts the decision and the exact reason.
  - Per rule: at least two matching commands or paths, including one PowerShell form and one Windows path with backslashes.
  - Review Focus 1, which must give no decision:
    - `git stash list`;
    - `git stash apply 1a2b3c`;
    - `git stash push -u -m "wip-x"`;
    - `rm -rf C:/Users/x/AppData/Local/Temp/claude/scratch/mh1`;
    - `Get-Content $env:TEMP\mh\run\endpoint.json`, where the scratch home isn't named `.marshell`;
    - `taskkill /PID 123 /T /F`;
    - `git commit` with only `.ts` staged, using a temp git repo fixture;
    - an edit to `src/lib/a.ts`;
    - an edit to `C:/dev/claude-setup/src/generated/x.ts`, outside a checkout of this repo, which is detected by the absence of `crates/protocol` at the root;
    - an edit to `~/.claude/projects/C--dev-Airport/memory/x.md`;
    - an edit to `~/.claude/settings.json`;
    - each of the four mention-only commands in Review Focus 1.
  - Review Focus 2:
    - stdin `""`, `"nope"` and `{}`;
    - `{"tool_name":"Bash"}` with no input.
    Each gives exit 0 and empty stdout.
  - Review Focus 3: `.claude/worktrees/x/src/generated/a.ts` gives F1 (its git root is the worktree), and `c:\DEV\Airport\SCOPE.md` gives F2.
- [ ] **Step 2: Run** `node --test .claude/hooks/`. Expected: FAIL (modules missing).
- [ ] **Step 3: Implement** `lib.mjs`, `guard-shell.mjs` and `guard-files.mjs`, and register them in `.claude/settings.json`:
  ```json
  "hooks": { "PreToolUse": [
    { "matcher": "Bash|PowerShell", "hooks": [{ "type": "command", "command": "node \"${CLAUDE_PROJECT_DIR}/.claude/hooks/guard-shell.mjs\"", "timeout": 15 }] },
    { "matcher": "Edit|Write|MultiEdit|NotebookEdit", "hooks": [{ "type": "command", "command": "node \"${CLAUDE_PROJECT_DIR}/.claude/hooks/guard-files.mjs\"", "timeout": 5 }] }
  ] }
  ```
- [ ] **Step 4: Run** `node --test .claude/hooks/`. Expected: PASS. Then `pnpm test:claude` gives the same result.
- [ ] **Step 5: Live-verify in a real session, with canaries only.** If a guard fails to load, a canary does nothing.
  - **Nesting first.** Run `claude -p "Reply with only: ok"` in this worktree. If it refuses to start inside a Claude Code session, don't unset anything to force it. Give the user the commands below to run in their own terminal, and record what they report.
  - **S1:** `claude -p "Run this exact Bash command: taskkill /IM marshell-canary-none.exe"`. Expected: Claude reports the S1 reason.
  - **S3:** `claude -p "Run this exact Bash command: git stash push"`. Run it only after `git status --porcelain` is empty in this worktree, so a failed guard stashes nothing. Expected: the S3 reason, and `git stash list` is unchanged.
  - **F1:** `claude -p "Use the Write tool to create src/generated/zz-canary.ts containing //"`. Expected: denied, and no file. Delete the file if it appears.
  - **The `ask` decision.** Check that S7's `ask` prompts under the user's `bypassPermissions`, using `git branch -D marshell-canary-none` on a branch that doesn't exist. If it doesn't prompt, change S7 and F3 to deny with ` Ask the user in chat first.` appended to the reason. Ledger this as a ruling.
- [ ] **Step 6: Commit** as `chore(claude): add guard hooks that enforce SCOPE.md and the live-check rules`.

### Task 3: Resume hook and path-scoped rules

**Files:**
- Create:
  - `.claude/hooks/session-start.mjs`, with tests in `.claude/hooks/session-start.test.mjs`
  - `.claude/rules/ui.md` (paths `src/**/*.ts`, `src/**/*.tsx`, `src/**/*.css`)
  - `.claude/rules/tauri-shell.md` (`src-tauri/**`)
  - `.claude/rules/local-api.md` (`crates/core/src/server/**`)
  - `.claude/rules/protocol.md` (`crates/protocol/**`)
  - `.claude/rules/pty.md` (`crates/core/src/pty/**`)
  - `.claude/rules/config-writes.md` (`crates/core/src/config/**`, `crates/core/src/agents/**`)
- Modify: `.claude/settings.json` (`hooks.SessionStart` with matcher `startup|resume|compact|clear`).

**The resume hook's output.** It prints `additionalContext`, at most 15 lines:
- `Branch <b>, <n> uncommitted files.`
- For each `.superpowers/sdd/*/progress.md` found in this checkout and in `<main>/.claude/worktrees/*`:
  `Ledger <relative path>: plan <basename>; last: <last "Task N: complete" line, cut to 160 chars>; next: Task <N+1>.`
- With no ledger, only the branch line.

**Rules content.** Each rule file holds 5–12 bullets taken only from `AGENTS.md`, `CLAUDE.md`, `SCOPE.md`, `docs/PLAN.md` and the newest plans. Each bullet names its source in parentheses. No new rules. For example, `ui.md` covers:
- keyboard only and forced colors;
- tokens in `src/styles`;
- read the good-css skill before writing declarations;
- no guessed status values;
- exact copy from the plan;
- components tested through `renderToStaticMarkup`;
- the xterm Ctrl+V and native-file-drop gotchas.

- [ ] **Step 1: Write the failing tests.**
  - `prints_branch_and_ledger_tail`: a temp repo with a ledger whose last lines are `Task 2: complete (…)`. The output contains `next: Task 3`.
  - `silent_without_ledger_beyond_branch`.
  - `caps_at_15_lines`: 20 ledgers.
  - `fails_open_outside_git`.
- [ ] **Step 2: Run** `node --test .claude/hooks/`. Expected: FAIL.
- [ ] **Step 3: Implement** the hook and register it. Write the six rule files.
- [ ] **Step 4: Run** `node --test .claude/hooks/`. Expected: PASS.
- [ ] **Step 5: Verify.** Run `claude -p "Read src/main.tsx, then say which project rule files are loaded"` in this worktree. Expected: `ui.md` is named, and `pty.md` is not.
- [ ] **Step 6: Commit** as `chore(claude): add a resume hook that points at the active plan ledger, and rules that load per area`.

### Task 4: Skills and the reviewer agent

**Files:**
- Create:
  - `.claude/skills/marshell-live-check/SKILL.md`
  - `.claude/skills/marshell-live-check/live-check.ps1`
  - `.claude/skills/marshell-codex-task/SKILL.md`
  - `.claude/skills/marshell-ship/SKILL.md`
  - `.claude/agents/marshell-reviewer.md`
- Modify: `CLAUDE.md`. "How a slice runs" and "Live check (Windows)" each gain one line naming the skill. Nothing is removed.

**`marshell-live-check`**
- Description: `Use when a plan step needs the real Marshell app (pnpm tauri dev): live checks, exit checks, reproducing a UI bug.`
- Body:
  - The `CLAUDE.md` procedure.
  - The memory lessons, now in the repo:
    - Sessions ignore a PATH shim, so set a scratch `CLAUDE_CONFIG_DIR`. Claude then stops at login; ask the user if the check needs a real answer.
    - `/model` and `/effort` write the real settings.
    - List the process tree and look for agent CLIs the user started before stopping anything.
    - Synthesized input verifies the target process and window rect.
    - CDP input reaches only the webview.
    - Never name the scratch home `.marshell`.
  - How to use the `webview` MCP: `browser_evaluate` returning small JSON, `browser_press_key` and `browser_emulate_media` (forced colors). Emulation lasts only while that one connection is open.
  - The research-note template.
- `live-check.ps1` has two modes:
  - `start -Scratch <literal path>` creates `mh` and `cc` under that path. It refuses an existing non-empty folder. It starts `pnpm tauri dev` with `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`. It writes `pids.json` with the root PID and prints it.
  - `stop -Scratch <path>` lists the tree under the recorded root PID, with name, PID and parent. It stops only those PIDs, leaves out any `claude.exe` or `codex.exe` the tree didn't start, and prints what it stopped. It never deletes the folder.

**`marshell-codex-task`**
- Description: `Use when handing a plan task to Codex CLI.`
- Body:
  - One worktree per task under `C:/dev/codex-wt/<slice>-t<N>`, branched from the slice branch.
  - The prompt template: the binding docs, "Task N only", TDD, the commands, the commit trailer and the final report.
  - Run it as:
    ```
    codex exec -C <wt> -s workspace-write -o <out>.md - < <prompt>
    ```
  - Its sandbox can't write the worktree's index, which lives in the main `.git`. Don't widen it with `--add-dir`, which would open every ref and the shared stash.
  - The prompt says to leave the changes uncommitted and to write the commit message to `.superpowers/codex-commit-message.txt` with UTF-8 and no BOM (not PowerShell 5's `Set-Content -Encoding utf8`). Claude commits after review, stripping a leading BOM before `git commit -F`.
  - Review: read the diff and run the tests yourself. Codex's sandbox can't run some core tests. Watch for CRLF and a UTF-8 BOM in messages written from PowerShell.
  - Cherry-pick, then `task-done`.
  - **Quota:** if the log shows `usage limit`, check the worktree. Codex may have finished; it did in 5b Task 2. If not, hand the task to a `worker` and tell the user.

**`marshell-ship`**
- Description: `Use before opening a pull request in this repo.`
- Body:
  - Run what CI runs, in order, with the exact commands from `ci.yml`, plus `pnpm test:claude`. Keep each tail.
  - The PR body template: What it does, Live check, Known limits, Checks (commands and counts), and the Claude Code footer.
  - Then wait for CI and merge. A failure on the known hook timing test `a_status_line_update_does_not_cancel_the_held_done` gets one rerun, and is named in the summary.

**`marshell-reviewer`**
- Frontmatter: `model: opus`, `effort: xhigh`, `tools: Read, Grep, Glob, Bash`.
- Description: `Final whole-branch review for a Marshell slice. Give it the review-package path, the plan path and the plan's Review Focus.`
- Body:
  - Binding docs.
  - Per-area checks:
    - the CSP and capabilities are unchanged;
    - routes are token-guarded, and ids are validated before any path;
    - blocking work runs in `spawn_blocking`;
    - pty and process-tree safety;
    - config writes follow the preview, backup, atomic-write and undo rules;
    - vendor code stays in adapters;
    - keyboard and forced colors;
    - exact copy;
    - the `src/generated` regeneration is committed.
  - Output: Critical, Important and Minor findings, each with file:line and a failure scenario, then "Declined to judge".

- [ ] **Step 1: Write the four files and the agent.** The shell commands in the skills must pass the Task 2 guards; for example, the stop step uses PIDs.
- [ ] **Step 2: Test `live-check.ps1`.**
  - Run `start` with a literal scratch path under the session scratchpad, wait for `http://127.0.0.1:9333/json/version`, then run `stop`.
  - **Expected:** the tree printed, every recorded PID gone, the scratch folder still there, and `~/.claude/settings.json` unchanged.
- [ ] **Step 3: Test the webview MCP in that same run,** using a `claude -p` session in this worktree: `browser_evaluate` of `document.title` gives `Marshell`.
- [ ] **Step 4: Verify loading.** `claude -p "List skills whose name starts with marshell- and agents whose name starts with marshell-"` lists all four.
- [ ] **Step 5: Commit** as `chore(claude): add live-check, Codex and ship skills and an Opus reviewer agent for this repo`.

### Task 5: Ship

- [ ] Run the `marshell-ship` checks: CI's list, plus `pnpm test:claude`.
- [ ] Do a whole-branch review with the new `marshell-reviewer` agent, followed by one fix pass.
- [ ] Open the PR.
  - "What it does" lists the pieces.
  - "Live check" gives the Task 1, 2 and 4 verifications with their numbers.
  - "Known limits" covers these three:
    - per-person MCP approvals in new worktrees go through `.worktreeinclude`;
    - worktrees cut before this merge don't have the setup;
    - the possible double `CLAUDE.md` load in `.claude/worktrees`;
    - path-scoped rules match paths under the session's own project root, so edits made from a main-checkout session into `C:/dev/codex-wt/*` don't load them;
    - `.worktreeinclude` affects only worktrees that Claude Code creates, not ones made with `git worktree add`.
- [ ] Wait for CI, then merge.
- [ ] Fast-forward the main checkout so `${CLAUDE_PROJECT_DIR}` hooks resolve there.
