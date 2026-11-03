# Qwen Code: sign-in, update, skills, transcripts, WSL and settings (slice 6l2 research)

- Date: 2026-11-03. A read of Qwen Code 0.26.5's bundled source, done for slice 6l2's design questions. Qwen was not run.
- Source: the standalone install in `%LOCALAPPDATA%\qwen-code\qwen-code` (`manifest.json` target `win-x64`, Node 22.23.4; `package.json` needs `node >=22`). The chunk names below are under `lib\chunks\`. This machine has no `~/.qwen`.
- Not verified here (left for the live check): `/doctor`'s output, a real transcript's first line, and what turns on "managed" sessions.

## Sign-in status

- There is no sign-in status command. `qwen auth` is still registered (`chunk-G3XDESK5.js`, `authCommand`), but its handler and the old subcommands (`status`, `coding-plan`, `openrouter`, `requesty`, `api-key`, `qwen-oauth`) print "qwen auth has been removed." and exit 0. The notice points to `/auth` in the TUI, to env vars, and to `/doctor`.
- `/doctor`'s "Authentication" check only asks whether the API client initialised (`checkApiClient`, `chunk-Q2SM4YRE.js`), so it doesn't say whether someone is signed in.
- The top-level commands are `mcp, memory, extensions, auth, hooks, channel, board, serve, daemon, attach, sessions, agents, batch, update, sandbox`, plus `review` and `autofix-pr` when named in argv. None of them logs in. "OAuth cannot be configured with env vars alone": login is `/auth` inside a session.
- The auth types are `qwen-oauth`, `openai`, `openai-responses`, `anthropic`, `gemini` and `vertex-ai`, in `security.auth.selectedType`. The bundle says "Qwen OAuth free tier was discontinued on 2026-04-15".
- OAuth credentials are kept in `<home>/oauth_creds.json` (with `oauth_creds.lock`). API keys are in `security.auth.apiKey` or in env vars (`OPENAI_API_KEY` with `OPENAI_BASE_URL`, `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`, `BAILIAN_CODING_PLAN_API_KEY`, `QWEN_OAUTH`).
- Slice 6l2 decision 1: Marshell shows only `security.auth.selectedType`, labelled as configured.

## Version and update

- `--version` and `-v` take a fast path (`hasVersionToken`; yargs has `.version(false)`) and print the bare version, `0.26.5`.
- The standalone layout is `manifest.json`, `bin/qwen.cmd`, `node/node.exe` and `lib/{cli-entry.js, cli.js, chunks}`. `getStandaloneInstallInfo` (`chunk-G2TIJFX2.js`) recognises it from `lib/cli.js` plus a `manifest.json` named `@qwen-code/qwen-code`. The targets are `darwin-arm64`, `darwin-x64`, `linux-arm64`, `linux-x64` and `win-x64`.
- `qwen update` ("Check for Qwen Code updates and install if available"; `--target-version <v>` works only on a standalone install) prints one of:
  - "Qwen Code {{version}} is up to date!"
  - "Failed to check for updates (…)" on stderr, exit 1.
  - On a standalone install, "Downloading update..." and then "Update successful! The new version will be used on your next run." or "Update downloaded. It will be applied after you exit this session."
  - "Update failed: …", exit 1.
- The update on exit (`qwen-code:update-on-exit`, `chunk-L5YQW6BG.js`, `update-relaunch-L57BZX5J.js`): on a standalone install, an in-process download from Alibaba's OSS bucket (or `QWEN_UPDATE_BASE_URL`, falling back to GitHub releases), checked by checksum and signature, then extracted next to the install under `.qwen-update.lock`. On npm, `npm install -g @qwen-code/qwen-code@latest`.
- Newer-version data is never kept on disk. The check is a live npm registry call (`checkForUpdatesDetailed`, `chunk-L3LOOW7R.js`). It is turned off by `general.enableAutoUpdate: false`, or for one run by `QWEN_CODE_SKIP_UPDATE_CHECK_ONCE`.
- The installers Qwen prints as the manual update command: `install-qwen-standalone.ps1` and `install-qwen-standalone.sh` under `https://qwen-code-assets.oss-cn-hangzhou.aliyuncs.com/installation/`.

## Skills

- Skills are on by default. `--experimental-skills` is hidden: "Skills are now enabled by default. This flag is ignored."
- The user folders (`Storage.getUserSkillsDirs`, `chunk-EPODBRBP.js`) are `<QWEN_HOME or ~/.qwen>/skills/<name>/SKILL.md` and `~/.agents/skills/<name>/SKILL.md`. `QWEN_HOME` doesn't move the second one, which is built from `os.homedir()`, and settings can add more folders. The project folders are `.qwen/skills` and `.agents/skills`.
- The file must open with `---` frontmatter, or loading fails with "Invalid format: missing YAML frontmatter". `name` and `description` are required, and `name` matches `/^[\p{L}\p{N}_:.-]+$/u`. A skill is model-invoked unless it sets `disable-model-invocation: true`. Skills also show as `/<name>` commands and are reloaded when they change.
- There is no size limit on `SKILL.md` itself. The skill listing is capped at 8000 characters and a name at 256.
- Not known: whether the same skill in both user folders is listed once or twice. The live check looks (slice 6l2 decision 3).

## Transcripts and home

- The home (`getGlobalQwenDir`) is `QWEN_HOME` itself when it is set, resolved with a leading `~` expanded; it is not `QWEN_HOME/.qwen`. Otherwise it is `~/.qwen`. Settings and credentials always live in the home.
- `projects/` is under `QWEN_RUNTIME_DIR`, or `advanced.runtimeOutputDir` in settings, and otherwise under the home.
- The project slug (`sanitizeCwd`, `chunk-OKN3Z5C5.js`) is the cwd with every `[^a-zA-Z0-9]` turned into `-`, lowercased on Windows only.
- A conversation is `projects/<slug>/chats/<sessionId>.jsonl`. Qwen's own lister accepts names matching `^[0-9a-fA-F-]{32,36}\.jsonl$`. Next to it in `chats/` are `<id>.runtime.json`, `.worktree.json`, `.pr.json` and an `archive/` folder. Subagents use `projects/<slug>/subagents/<id>/`.
- The file is append-only: it is created with `wx`, then each record is appended as one JSON line. Records chain by `parentUuid`, and `rewind` and `branch_checkpoint` records exist.
- Every record has `uuid`, `parentUuid`, `sessionId`, `timestamp` (ISO), `type`, `provenance`, `cwd`, `version` and `gitBranch`. `type` is one of:
  - `user`: `message: {role: "user", parts: […]}`, with optional `promptId`, `systemPayload` and `executionContext`.
  - `assistant`: `message: {role: "model", parts: […]}`, with `model`, `usageMetadata` and `contextWindowSize`.
  - `tool_result`.
  - `system`: carries a `subtype`, one of `custom_title`, `chat_compression`, `session_model`, `ui_telemetry`, `parent_session` or `managed_session_header_v1`.
- The title is the last `custom_title` record's `systemPayload.customTitle` (`titleSource` `auto` or `manual`). Without one, the first user prompt (cut to 200 characters) is the preview. Qwen's own session list uses the first record's `cwd`.
- `qwen sessions list --json` prints JSON Lines for the current folder's project only (`sessionId, startTime, mtime, prompt, gitBranch, customTitle, titleSource, filePath, cwd`).

## Linux and WSL

- The npm package `@qwen-code/qwen-code` (`bin.qwen` is `cli-entry.js`) and standalone `linux-x64` and `linux-arm64` builds exist.
- Off Windows, hooks run through `bash -c` and the command is passed verbatim (`getShellConfiguration`, `expandCommand`), so the Windows quoting doesn't apply. A hook may also set `shell: "bash"` or `"powershell"`. Every hook gets `QWEN_PROJECT_DIR`, `CLAUDE_PROJECT_DIR`, `GEMINI_PROJECT_DIR` and `QWEN_CODE_SESSION_ID`.

## Settings

- The system file is `C:\ProgramData\qwen-code\settings.json` on Windows, `/Library/Application Support/QwenCode/settings.json` on macOS and `/etc/qwen-code/settings.json` on Linux. `QWEN_CODE_SYSTEM_SETTINGS_PATH` and `QWEN_CODE_SYSTEM_DEFAULTS_PATH` override those paths. Layers apply in the order system defaults, user, workspace (`.qwen/settings.json`), then system, so the system file wins.
- The format is JSONC: `parseTree` with `allowTrailingComma: true`. Qwen keeps a file's format when it writes (`updateSettingsFilePreservingFormat`, through a temp file and a `.orig` backup), refuses a file it can't parse, and adds `"$version": 4` on first run.
- The keys slice 6l2's form uses:

  | Key | Type | Default |
  |---|---|---|
  | `model.name` | string | |
  | `tools.approvalMode` | `plan`, `default`, `auto-edit`, `auto`, `yolo` | `auto` in the schema |
  | `general.enableAutoUpdate` | boolean | true |
  | `privacy.usageStatisticsEnabled` | boolean | true |
  | `ui.theme` | string | "Qwen Dark" |
  | `general.vimMode` | boolean | false |

- Other keys of note:
  - `general`: `chatRecording` (true), `cleanupPeriodDays` (30) and `notificationMode` (`all` or `task-complete`).
  - `ui`: `statusLine` and `accessibility.screenReader`.
  - `model`: `reasoningEffort` (`low` to `max`).
  - `security`: `folderTrust.enabled` (false).
  - Other top-level keys: `disableAllHooks`, `hooks`, `permissions`, `skills`, `mcpServers`, `modelProviders` and `env`.
- Keys that can hold a secret: `security.auth.apiKey`, everything under `env`, the entries of `modelProviders`, `mcpServers.*.env` and `mcpServers.*.headers`, a hook's `env`, and `telemetry` headers.

## Transcripts seen (2026-11-03, slice 6l2 task 1)

Qwen Code 0.26.5 run headless (`qwen -p`) against the fake model, with `USERPROFILE` and `HOME` in a scratch folder and `QWEN_HOME` unset. The scrubbed files are in `crates/core/tests/fixtures/qwen/transcripts/0.26.5/`, each named after its (scrubbed) `sessionId`:

| File (`00000000-0000-4000-8000-…`) | Prompt | What it holds |
| --- | --- | --- |
| `…002` | `hello`, then `--resume <id> -p "hello again"` | Two text turns in one file |
| `…026` | `WRITE notes.txt` (a relative path) | A tool refused before it ran (`errorType: invalid_tool_params`) |
| `…046` | `hello from a repo` | A chat in a git repo, `gitBranch: "feature-x"` |
| `…059` | `RUN mkdir made` | `run_shell_command`, success |
| `…079` | `READ <abs> ;; EDIT <abs>` | `read_file` then `edit`, both success, with a `fileDiff` |
| `…107` | `RUN exit 3` | `run_shell_command`, error, `exitCode: 3` |
| `…131` | `WRITE <abs>` | `write_file` creating a file, with a `fileDiff` |

- A headless run writes `projects/<slug>/chats/<sessionId>.jsonl` exactly as an interactive one does. The first line is the first `user` record, carrying `sessionId`, `cwd`, `version` and `executionContext` (`modelId`, `authType`, `approvalMode`).
- `provenance` doesn't tell headless from interactive: a headless prompt is `real_user`. The source sets it from the record type alone (`createBaseRecord`: `user` → `real_user`, `assistant` → `assistant_output`, and so on), and the attribution snapshot's `surface` is `QWEN_CODE_ENTRYPOINT`, else `cli`, in both modes. So by the source, nothing in the file tells a `qwen -p` run from a terminal one. An interactive file was not recorded here; the live check compares one.
- No `custom_title` record appears on a headless run.
- `--resume <id>` appends to the same file; the new prompt's `promptId` counts on (`<id>########2`).
- A tool call is the assistant's `functionCall {id, name, args}`. Its result is a `tool_result` record whose `functionResponse.id` and `toolCallResult.callId` both equal that id. `toolCallResult.status` is `success` or `error`. A failed call's `functionResponse.response` has `error` in place of `output`.
- `toolCallResult.resultDisplay` is a string for read and refused calls, a `shell_result` object for the shell (`exitCode`, `outcome` `completed` or `failed`), and for `write_file` and `edit` an object with `fileDiff`, `fileName`, `filePath`, `originalContent` and `newContent`.
- `gitBranch` is absent outside a git repo and on a branch with no commit yet.
- `system` subtypes seen: `ui_telemetry`, `attribution_snapshot` and `memory_catalog_snapshot` (the last carries a `message` with role `user`, so a reader must key on `type`, not on the role).
- The tools refuse a relative path, and `edit` refuses a file not read in the same process, even across `--resume`. The recording used a scratch copy of the fake that walks several commands in one prompt (separated by ` ;; `) to get a read and an edit into one run.
