# Marshell research: accounts, cloud conversations, extensions, updates (2026-10-16)

Scope: Claude Code 2.1.301, Codex CLI 0.162.1, Antigravity CLI (`agy`) 1.3.3, all installed and run on this Windows 11 machine. Marks: VERIFIED = primary vendor source or a command run here; UNVERIFIED = inference or secondary source. "Local" means observed on this machine by running a command or listing a directory (no credential file was read). Already covered in `docs/research/2026-10-10-cli-data-sources.md` and `docs/research/codex-cli.md` and not repeated: status-line push channels, hook events, hook trust, AGENTS.md chain, app-server rate-limit methods.

Primary doc hosts: `code.claude.com/docs/en/*` (Claude), `learn.chatgpt.com/docs/*` (Codex; `developers.openai.com/codex/*` redirects there), `antigravity.google/docs/*` (agy).

---

## 1. Third-party sign-in (terms, hosted login, API key)

### Claude Code
| Point | Finding | Mark | Source |
|---|---|---|---|
| (a) Terms on subscription OAuth in third-party apps | OAuth is "intended exclusively for purchasers of Claude Free, Pro, Max, Team, and Enterprise subscription plans" and for "ordinary use of Claude Code and other native Anthropic applications". Products (including Agent SDK) "should use API key authentication through Claude Console or a supported cloud provider". "Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their users." Developers "may not collect, store, or intermediate Claude.ai credentials or session tokens"; sign-in "must complete through Anthropic's own flow". | VERIFIED | https://code.claude.com/docs/en/legal-and-compliance (Authentication and credential use) |
| (b) Hosting the CLI's own login in a terminal | Explicitly allowed: "an end user [signing] in to the unmodified Claude Code binary with their own Claude subscription, including where a platform hosts Claude Code". Conditions: binary unmodified, no auth method removed or restricted, nobody pays for or resells Claude usage for end users, each user authenticates with their own key/subscription/3P credential, no Anthropic names or logos in a product name. Anthropic may enforce without notice. | VERIFIED | same page |
| (c) API-key path | Allowed and the recommended route for products: own API key, `apiKeyHelper`, `ANTHROPIC_API_KEY` (prompts once in interactive mode), Bedrock/Vertex/Foundry env. Console sign-in without a key (v2.1.242+) stores a profile in `%APPDATA%\Anthropic`. Precedence: cloud provider vars, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`, `apiKeyHelper`, `CLAUDE_CODE_OAUTH_TOKEN`, profiles, subscription OAuth. | VERIFIED | https://code.claude.com/docs/en/authentication |
| Extra | `claude setup-token` prints a one-year OAuth token for `CLAUDE_CODE_OAUTH_TOKEN` (not saved; no Remote Control, no claude.ai connectors). Handling that token in Marshell would count as storing a Claude.ai credential, so avoid. | VERIFIED (doc) / UNVERIFIED (reading as a Marshell violation) | authentication page |
| Secondary | Press reports of a Feb 2026 terms update banning subscription tokens in third-party tools; date not confirmed. | UNVERIFIED | aicoder/mindstudio search results |

### Codex CLI
| Point | Finding | Mark | Source |
|---|---|---|---|
| (a) Terms | The CLI auth page covers only the two methods (ChatGPT sign-in, API key) and says "Treat `~/.codex/auth.json` like a password". It does not address a third-party app reading or reusing those tokens. OpenAI's sanctioned route for third-party apps using a ChatGPT plan is "Sign in with ChatGPT" (SIWC): available "to selected commercial partners through a limited trial"; "ChatGPT plan usage is available to all open-source partners and selected private clients"; open-source and locally hosted apps use the token-sharing flow, paid or remotely hosted apps must complete an interest form; button must read "Continue with ChatGPT"; plan usage "does not grant access to their ChatGPT conversations". Flow is auth code + PKCE, loopback on `127.0.0.1`, `client_id=dynamic_agent_client` for first registration, scopes `openid profile email offline_access resource.invoke chatgpt.tokens.use.direct`, per-client credentials stored 0600, written atomically, never logged. | VERIFIED | https://learn.chatgpt.com/docs/auth ; https://developers.openai.com/siwc/quickstart ; https://developers.openai.com/siwc/token-sharing-open-source ; https://developers.openai.com/siwc/token-sharing-open-source/sign-in ; https://learn.chatgpt.com/docs/sign-in-with-chatgpt |
| (a) cont. | Reported: on 2026-09-08 the Codex lead said third-party clients using SIWC for Codex are not approved unless pure open-source or existing partners; others must contact OpenAI. Help article and DevDay recap returned 403 to me. | UNVERIFIED (secondary: https://www.aicoder.com/news/news-20260908-openai-codex-sign-in-policy ; https://xenospectrum.com/en/chatgpt-sign-in-subscription-apps/) | |
| (b) Hosting `codex login` | Nothing in the docs forbids a user running the unmodified CLI in any terminal; `codex login`, `--device-auth` (beta, headless), `--with-access-token` are documented, the last "for trusted scripts/CI; admins must permit". Safe path = host the real CLI, never touch `auth.json`. The "safe" judgement itself is an inference. | VERIFIED (commands) / UNVERIFIED (judgement) | https://learn.chatgpt.com/docs/auth |
| (c) API key | `codex login --with-api-key` (key on stdin). Local work only; Codex cloud needs ChatGPT sign-in. | VERIFIED | auth page ; local `codex login --help` |

### Antigravity CLI (agy)
| Point | Finding | Mark | Source |
|---|---|---|---|
| (a) Terms | Terms s.6: "Using third party software, tools, or services to access the Service" is "a breach of this Agreement" (example: OpenClaw with Antigravity OAuth), grounds for "suspension or termination of your Antigravity and/or Gemini CLI accounts". FAQ: "Using third-party software, tools, or services to access Antigravity is a violation of our Terms of Service"; recommends a Gemini Enterprise or Google AI Studio key for third-party agents; "`agy -p` is intended for direct scripting and CI workflows. Wrapping `agy` in third-party agents or automating account rotation is not supported on consumer plans." | VERIFIED | https://antigravity.google/terms ; https://antigravity.google/docs/faq |
| (a) cont. | Feb 2026 ban wave over OpenClaw OAuth, partly reversed 2026-02-27. | UNVERIFIED | https://gigazine.net/gsc_news/en/20260224-google-gemini-openclaw-accounts-banned ; https://metricnexus.ai/blog/google-banning-openclaw-antigravity-users |
| (b) Hosting `agy` | A user running the unmodified `agy` in a terminal is not "third-party access" on a plain reading, but Google does not bless it explicitly and the FAQ line about "wrapping agy in third-party agents" is a risk flag for any automation layer (injecting prompts, rotating accounts). Keep Marshell to a plain pty. | UNVERIFIED (interpretation) | FAQ |
| Login mechanics | No `login`/`auth`/`status`/`doctor` subcommand (local: `agy login --help` prints main usage). First `agy` launch with no saved session opens the browser (personal Google account); Business accounts use Google Cloud/WIF; remote/SSH prints a URL and you paste the code back; `/logout` is in-TUI. | VERIFIED | https://antigravity.google/docs/install?tab=cli ; local |
| (c) API key | `GEMINI_API_KEY` AND `modelProvider: "gemini"` in `settings.json`; a bare env var does nothing; `GOOGLE_API_KEY` and `.env` are not read. ADC: `gcloud auth application-default login` plus `AGY_ADC_AUTH=true`. | VERIFIED | install page |

---

## 2. Sign-in status without touching secrets

| | Claude Code | Codex CLI | Antigravity |
|---|---|---|---|
| Status command | `claude auth status` (exit 0 logged in, 1 not) | `codex login status` (plain text); `codex doctor --json` check `auth.credentials` | None. Only the statusLine stdin JSON (`email`, `plan_tier`, `quota`) while a session runs; headless run fails with "authentication required" if no cached login |
| Machine-readable | JSON by default (`--text` for prose) | `codex login status` text only ("Logged in using ChatGPT"); doctor is JSON | n/a (statusLine JSON only inside a session) |
| Fields | `loggedIn`, `authMethod`, `apiProvider`, `email`, `orgId`, `orgName`, `subscriptionType` (local: "max"), `projectsDirectory`, `configDirectory`, `analyticsDisabled`. `authMethod` documented values: `none`, `claude.ai`, `oauth_token`, `api_key`, `api_key_helper`, `third_party` | Doctor `auth.credentials` details: auth file path, storage mode ("File"), stored API key yes/no, stored ChatGPT tokens yes/no, stored agent identity, stored auth mode ("chatgpt"). No email, no plan in this check | statusLine: `email`, `plan_tier`, per-model `quota` (`remaining_fraction`, `reset_time`) |
| Credential storage | Windows `%USERPROFILE%\.claude\.credentials.json`; macOS Keychain (fallback `~/.claude/.credentials.json`); Linux `~/.claude/.credentials.json` (0600). `CLAUDE_CONFIG_DIR` relocates | `~/.codex/auth.json` or OS keyring; setting `cli_auth_credentials_store` = `file`, `keyring`, `auto`, `ephemeral` (local doctor: File) | OS keyring (Windows Credential Manager, macOS Keychain, Linux Secret Service). Legacy Gemini CLI files (`oauth_creds.json`) still sit in `~/.gemini/` locally but are not documented for agy |
| Expiry exposed? | No field in `auth status`. Only: startup warning within 3 days of expiry of a `/login` login ("Your login expires in 3 days"), failure text "Login expired · Please run /login", and the in-session `/status` row "Expired — log in again" (v2.1.210+) | No. Docs: "Codex refreshes tokens automatically during use before they expire", no lifetime given. SIWC sample `expires_in` 3600 is an example only | No |
| Real re-login time knowable? | No. Refresh is automatic and the refresh/session lifetime is undocumented. Marshell can show "signed in as X (plan)" and react to the "Login expired" text or a failing `auth status`, but cannot forecast. | No (same). React to `codex login status` failing or doctor reporting no stored tokens | No (same) |
| Marks | VERIFIED (docs + local run) | VERIFIED (docs + local run) | VERIFIED (docs); absence of a status command VERIFIED locally |

Sources: https://code.claude.com/docs/en/cli-reference ; https://code.claude.com/docs/en/authentication ; https://learn.chatgpt.com/docs/auth ; https://antigravity.google/docs/install?tab=cli ; https://antigravity.google/docs/cli/statusline.

---

## 3. Cloud-only conversations (can a local app list them?)

| Surface | Listable by a local app? | Mark | Source |
|---|---|---|---|
| Claude Code web sessions (claude.ai/code) | Only inside the CLI: `claude --teleport` (interactive picker), `claude --teleport <id>`, `/teleport`, `/tasks`. Start/follow-up: `claude --cloud "task"` (`--remote` deprecated), `claude -p "msg" --cloud <session-id>` with `--output-format json` returning `{ok,session_id,url}`. No documented list command or public list API. Needs claude.ai sign-in (no API key / 3P). Teleport needs clean git, right repo, pushed branch, same account. | VERIFIED (commands, no documented list API) / UNVERIFIED (that the picker uses a private endpoint) | https://code.claude.com/docs/en/claude-code-on-the-web |
| Org-wide transcripts | Enterprise Compliance API lists chats (`GET /v1/compliance/apps/chats`) and session transcripts; org admin only, not for an individual user's app. | VERIFIED (exists, search summary of platform docs) | https://platform.claude.com/docs/en/api/compliance/apps/chats ; https://platform.claude.com/docs/en/manage-claude/compliance-sessions |
| claude.ai chat history | No public API for an individual account. Only in-app export (not verified here). Scraping would be "intermediating session tokens", barred by the terms above. | UNVERIFIED (absence) | search of platform/support docs |
| Claude Desktop / VS Code sessions | Each keeps its own list; not shared with the CLI picker. | VERIFIED | claude-code-on-the-web page |
| Codex cloud tasks | Yes, via CLI (experimental): `codex cloud list [--env] [--limit 1-20] [--cursor] [--json]`, also `status`, `diff`, `apply`, `exec`. Local run: `codex cloud list --limit 1` returned "No tasks found." JSON shape not yet seen (no tasks). Docs say the CLI can start a task and show recent cloud chats. Needs ChatGPT sign-in. No documented SDK/REST list API. | VERIFIED (local help + run; docs) | https://learn.chatgpt.com/docs/cloud.md |
| ChatGPT history | No. SIWC explicitly "does not grant access to their ChatGPT conversations". | VERIFIED | https://developers.openai.com/siwc/token-sharing-open-source |
| Antigravity | No cloud conversation listing documented; sessions are local (`conversations/<uuid>.db`). | UNVERIFIED (absence) | docs index https://antigravity.google/llms.txt |

---

## 4. Local transcripts

| | Claude Code | Codex CLI | Antigravity |
|---|---|---|---|
| Location | `~/.claude/projects/<project>/<session-id>.jsonl`; `<project>` = cwd with non-alphanumerics replaced by `-`, truncated to 200 chars plus hash. Overrides `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_PROJECT_DIR_NAME`. Also `~/.claude/sessions/`, `history.jsonl` | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` (local: 2026/10 present), plus sqlite: `state_5.sqlite`, `thread_history_1.sqlite`, `logs_2.sqlite`, `goals_1`, `memories_1`, `queue_1`. `codex migrate-rollouts` moves rollouts into `thread_history_1.sqlite`. `history.jsonl` also present | `~/.gemini/antigravity-cli/conversations/<uuid>.db` (sqlite: `trajectory_meta`, `steps`, `gen_metadata`, `executor_metadata`, `parent_references`, ...; blobs look like protobuf), `conversation_summaries.db`, `history.jsonl` (slash-command log only), `cache/last_conversations.json` (workspace path to conversation id) |
| Format stability | JSONL; docs say entry format "is internal... changes between versions" | JSONL migrating to sqlite; version-fragile (already in repo notes) | Internal sqlite/protobuf, not a documented interface |
| Retention | 30 days default (`cleanupPeriodDays`) | not documented in pages read | not documented |
| Resume | `claude --resume [id\|name\|path.jsonl]`, `--continue`, `--fork-session`, `/resume`, `--from-pr`. `claude -p`/SDK sessions are hidden from the picker | `codex resume [SESSION_ID] [PROMPT]`, `--last` (scoped to cwd unless `--all`), `codex fork`; `codex archive/unarchive/delete` | `agy -c/--continue` (latest for cwd), `agy --conversation <id>`, `/resume` (aliases `/switch`, `/conversation`), `/fork`, `/rewind`, `/rename` |
| Safer structured access | `claude -p --output-format json\|stream-json`; hooks/statusLine `transcript_path`; `/export` | hooks `transcript_path`; app-server | headless `--output-format json\|stream-json` (`conversation_id`, `usage`, ...); statusLine `transcript_path` |
| Marks | VERIFIED https://code.claude.com/docs/en/sessions | Paths VERIFIED locally; resume VERIFIED https://learn.chatgpt.com/docs/developer-commands.md?surface=cli ; the `sessions/` path is not in the docs I read, so doc status UNVERIFIED | Resume VERIFIED https://antigravity.google/docs/cli/conversations ; db layout VERIFIED locally, docs silent on storage |

Recommendation for Marshell: list sessions by asking each CLI (`claude agents --json`, `codex resume` picker flows, `agy` last_conversations cache), keep transcript parsing optional, best-effort and version-gated.

---

## 5. Doctor commands

| CLI | Command | Non-interactive? | JSON? | Notes |
|---|---|---|---|---|
| Claude | `claude doctor` | Yes, read-only, no session started | No (plain text; no `--json`) | Local output includes version, install method ("Config install method: native"), "Auto-updates: enabled", update channel, "Last update attempt: success -> 2.1.301 (date)", managed settings, Remote Control eligibility, "No installation issues found." `/doctor` in-session is the fuller checkup that proposes fixes. VERIFIED (local + https://code.claude.com/docs/en/setup) |
| Codex | `codex doctor` | Yes | Yes: `--json` ("redacted machine-readable report"); also `--summary`, `--all`, `--no-color`, `--ascii` | Schema: `schemaVersion` 1, `generatedAt`, `overallStatus`, `codexVersion`, `checks{id:{id,category,status,summary,details,remediation,durationMs}}`, 25 checks locally. Includes `installation` (install context, PATH hits), `updates.status` (latest version, update action), `auth.credentials`, `config.load`, `mcp.config`, `app_server.status`, `state.paths`, `network.*`, `sandbox.*`. VERIFIED (local; docs https://learn.chatgpt.com/docs/developer-commands.md?surface=cli) |
| agy | none | n/a | n/a | `agy doctor --help` just prints main usage. Troubleshooting page documents keyring errors and `update.lock`; logs at `~/.gemini/antigravity-cli/log/cli-*.log` (observed locally, not stated in docs). VERIFIED (absence) https://antigravity.google/docs/cli/troubleshooting |

---

## 6. Portability of MCP, skills, hooks, plugins, slash commands

### MCP
| | Claude Code | Codex | agy |
|---|---|---|---|
| Files | `~/.claude.json` (local and user scopes), `<project>/.mcp.json` (project scope, shared via VCS), plugin `.mcp.json` | `~/.codex/config.toml` and project `.codex/config.toml` (trusted only), table `[mcp_servers.<name>]`; plugin servers only toggled via `plugins.<plugin>.mcp_servers.<server>` | global `~/.gemini/config/mcp_config.json`, workspace `.agents/mcp_config.json`; both hold an `mcpServers` object |
| Shape | JSON `mcpServers`; `type` http/sse/stdio, `command`/`args`/`env`, remote `url` | TOML: `command`/`args`/`env`/`env_vars`/`cwd`; remote `url` + `bearer_token_env_var`; `enabled_tools`/`disabled_tools` | JSON; stdio `command`/`args`/`env`/`cwd`; remote must use `serverUrl` (legacy `url`/`httpUrl` not supported); `disabled`, `disabledTools`, `headers` |
| CLI | `claude mcp add/add-json/list/get/remove`, `--scope local\|project\|user`, `claude mcp add-from-claude-desktop` (macOS and WSL only) | `codex mcp add/list/get/remove/login/logout` (`list --json`) | `agy mcp add/remove/list/enable/disable` (local help); the docs page does not document `agy mcp add`; `/mcp` manager in TUI |
| Shareable? | Stdio entries are near-identical JSON between Claude `.mcp.json` and agy `mcp_config.json`; Codex needs TOML translation; remote entries differ (`url` vs `serverUrl`). No CLI reads another CLI's file. Marshell can translate but must write each CLI's own file via its config service. | | |
| Marks | VERIFIED https://code.claude.com/docs/en/mcp | VERIFIED https://learn.chatgpt.com/docs/extend/mcp.md | VERIFIED https://antigravity.google/docs/mcp |

### Skills (SKILL.md)
| | Claude Code | Codex | agy |
|---|---|---|---|
| User scope | `~/.claude/skills/<name>/SKILL.md` | `$HOME/.agents/skills` (docs); local `~/.codex/skills` also exists (`synced`, `underwater-photoshop`) but is not in the docs I read | `~/.gemini/antigravity-cli/skills/<name>/` |
| Project scope | `.claude/skills/` | `.agents/skills` from cwd up to repo root | `<root>/.agents/skills/<name>/` (back-compat `.agent/skills`) |
| Other | plugin `skills/`, enterprise, claude.ai account skills; `.claude/commands/*.md` merged into skills | `/etc/codex/skills` (admin), bundled system skills; optional `agents/openai.yaml` for UI metadata; `[[skills.config]]` disables one | plugin `plugins/<name>/skills/` |
| Frontmatter | `name`, `description`, plus many Claude-only keys (`allowed-tools`, `model`, `context`, `hooks`, `paths`...) | `name` and `description` required | `description` required, `name` defaults to folder |
| Standard | all three cite the Agent Skills open standard (agentskills.io) | | |
| Shareable? | Same SKILL.md body format everywhere (VERIFIED). Project-level `.agents/skills` is a real shared location for Codex and agy (VERIFIED both). Claude's docs do not mention `.agents/skills` or `~/.agents/skills`, so assume Claude cannot see it (UNVERIFIED, probably no). User-level paths all differ, so sharing means copying or symlinking into three folders. `claude import codex\|gemini\|cursor [--dry-run]` (v2.1.213+) pulls other agents' config into Claude, one way (VERIFIED, local help). | | |
| Sources | https://code.claude.com/docs/en/skills | https://learn.chatgpt.com/docs/build-skills.md | https://antigravity.google/docs/skills |

### Hooks
| | Claude | Codex | agy |
|---|---|---|---|
| Location | `hooks` in settings.json (user, project, local, managed), plugin `hooks/hooks.json` | `hooks.json` or `config.toml` under `~/.codex`, project `.codex/`, plugins; trust by hash in `/hooks` | project `.agents/hooks.json`, global `~/.gemini/config/hooks.json` or inside `~/.gemini/antigravity-cli/settings.json`, plugin `hooks.json` |
| Events | many (see Claude hooks doc) | 12 (repo notes) | `SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`; default timeout 30 s; camelCase JSON fields |
| Shareable? | No. Wrapper and field styles differ: agy files are keyed by a hook name first, then event; Codex and Claude both nest matcher groups with `hooks` arrays and look alike, but events, payloads and trust rules differ. Treat as per-CLI. Equivalence of Claude and Codex handler shapes is an inference (UNVERIFIED). | | |
| Marks | VERIFIED https://code.claude.com/docs/en/hooks (repo notes) | VERIFIED https://learn.chatgpt.com/docs/hooks | VERIFIED https://antigravity.google/docs/hooks |

### Plugins
| | Claude | Codex | agy |
|---|---|---|---|
| Manifest | `.claude-plugin/plugin.json` (only `name` required; optional entirely), components at plugin root: `skills/`, `commands/`, `agents/`, `hooks/hooks.json`, `.mcp.json`, `.lsp.json`, `output-styles/`, `bin/`, `settings.json` | Components: skills, MCP, apps/browser extensions, hooks. Manifest name/location not stated in the pages I read (UNVERIFIED) | `~/.gemini/antigravity-cli/plugins/<name>/` with skills, agents, rules, MCP, `hooks.json`; `agy plugin validate` (adds `--strict`, `--json` in 1.4.0 notes) |
| CLI | `claude plugin list --json`, `install`, `validate`, marketplaces | `codex plugin add/list/marketplace/remove`; `/plugins` in TUI; ChatGPT and Codex share one public catalog; hook scripts are not deployed by install | `agy plugin list`, `validate` |
| Shareable? | No. Three different manifests and marketplaces; no cross-compatibility statement found in any of the three docs. | | |
| Sources | https://code.claude.com/docs/en/plugins-reference | https://learn.chatgpt.com/docs/plugins.md | https://antigravity.google/docs/plugins ; https://antigravity.google/docs/cli/commands/changelog (via `agy changelog` locally) |

### Slash commands and prompts
- Claude: custom commands are `.claude/commands/*.md`, now merged into skills; plugin `commands/`. VERIFIED.
- Codex: custom prompts (`~/.codex/prompts`) are "Deprecated. Use skills for reusable prompts." (docs index, VERIFIED); local `~/.codex/prompts` does not exist. Built-in slash commands only otherwise.
- agy: slash commands are built in (`/skills`, `/mcp`, `/hooks`, `/model`, `/effort`, `/statusline`, `/resume`, `/fork`...); user-defined command files are not documented. Skills are the extension path. VERIFIED (absence UNVERIFIED).
- Portability: none. Skills are the only cross-CLI unit.

---

## 7. Settings surface

| | Claude Code | Codex | agy |
|---|---|---|---|
| User file | `~/.claude/settings.json` (Windows `%USERPROFILE%\.claude`); also `~/.claude.json` (sign-in session, MCP, per-project state, global keys like `autoConnectIde`, `copyOnSelect`, `diffTool`) | `~/.codex/config.toml`; profiles `$CODEX_HOME/<profile>.config.toml`; project `.codex/config.toml` (trusted; `model_provider`, `notify`, `profile`, `otel` ignored there); managed `requirements.toml` (Windows `%ProgramData%\OpenAI\Codex\requirements.toml`) | `~/.gemini/antigravity-cli/settings.json` (only non-default values written), `keybindings.json`; no documented workspace settings file |
| Scopes/precedence | managed, `--settings`, `.claude/settings.local.json`, `.claude/settings.json`, user | CLI `-c` overrides, profile, project, user, managed | single user file |
| Schema | `$schema`: `https://json.schemastore.org/claude-code-settings.json` | JSON schema at `https://learn.chatgpt.com/docs/config-schema.json` (editor example uses `https://developers.openai.com/codex/config-schema.json`); source in repo `openai/codex` at `codex-rs/core/config.schema.json` (file exists, confirmed via gh); TOML uses a `#:schema <url>` comment, no `$schema` key | none |
| Key groups | model/effort (`model`, `effortLevel`, `fallbackModel`, `availableModels`), permissions (`permissions.allow/ask/deny/defaultMode`), `hooks`, MCP allow/deny lists, plugins (`enabledPlugins`, `extraKnownMarketplaces`, `pluginConfigs`), skills (`skillOverrides`), auth (`apiKeyHelper`, `forceLoginMethod`, `forceLoginOrgUUID`), `env`, `statusLine`, retention (`cleanupPeriodDays`), updates (`autoUpdatesChannel`, `minimumVersion`), `sandbox.*`, UI (`theme`, `viewMode`, `editorMode`, `defaultShell`) | `model`, `model_reasoning_effort`, `approval_policy`, `sandbox_mode`, `mcp_servers`, `hooks`, `skills`, `plugins`, `marketplaces`, `features`, `agents`, `notify`, `tui`, `history`, `project_doc_max_bytes`, `check_for_update_on_startup`, `cli_auth_credentials_store`, `forced_login_method`, `projects`, `profile` | documented: `toolPermission`, `artifactReviewPolicy`, `enableTerminalSandbox`, `allowNonWorkspaceAccess`, `altScreenMode`, `colorScheme`, `runningLightSpeed`, `verbosity`, `editor`, `editorMode`, `notifications`, `useG1Credits`, `enableTelemetry`, `showTips`, `showFeedbackSurvey`; also used: `modelProvider`, `statusLine`, `model`, `permissions`, `trustedWorkspaces`, `dangerouslySkipPermissions` (seen locally); edit via `/config` or `/settings` |
| Marks | VERIFIED https://code.claude.com/docs/en/settings ; https://code.claude.com/docs/en/settings-reference | VERIFIED https://learn.chatgpt.com/docs/config-file/config-reference.md (first ~100k of 136k chars read) | VERIFIED https://antigravity.google/docs/settings |

Claude Desktop is separate from Claude Code. Desktop (chat app) config is `claude_desktop_config.json` at `%APPDATA%\Claude\` (Windows) or `~/Library/Application Support/Claude/` (macOS), opened via Settings > Developer > Edit Config; logs in `%APPDATA%\Claude\logs`. VERIFIED: https://modelcontextprotocol.io/docs/2026-07-28/develop/connect-local-servers . The page says the Desktop settings window "is separate from your Claude account settings". Claude Code reads none of this file except through the one-time import `claude mcp add-from-claude-desktop` (macOS/WSL only; https://code.claude.com/docs/en/mcp, VERIFIED). The reverse (Desktop Code tab reading `~/.claude.json` and `.mcp.json`, and the Desktop file winning on name clashes) comes from a search summary and is UNVERIFIED. Bridges seen: setting `desktopSessionCleanupPeriodDays` and the `--desktop` flag. Marshell must not edit `claude_desktop_config.json` as if it were Claude Code config.

---

## 8. Version, install method, update, latest-version check

| | Claude Code | Codex CLI | Antigravity (agy) |
|---|---|---|---|
| Read installed version | `claude --version` prints `2.1.301 (Claude Code)`. Parseable with a regex on the leading semver. VERIFIED (local; setup page) | `codex --version` prints `codex-cli 0.162.1`. Parseable. `codex doctor --json` also has `codexVersion`. VERIFIED (local) | `agy --version` prints bare `1.3.3`. Parseable. VERIFIED (local) |
| Official install methods | Native installer (`irm https://claude.ai/install.ps1 \| iex`, binary at `%USERPROFILE%\.local\bin\claude.exe`, versions under `~\.local\share\claude\versions`); npm `@anthropic-ai/claude-code`; WinGet `Anthropic.ClaudeCode`; Homebrew casks `claude-code` / `claude-code@latest`; apt/dnf/apk. VERIFIED https://code.claude.com/docs/en/setup | Installer script (`irm https://chatgpt.com/codex/install.ps1 \| iex`; env `CODEX_RELEASE`, `CODEX_NON_INTERACTIVE`), npm `@openai/codex`, Homebrew cask `codex`. Page does not list winget. VERIFIED https://learn.chatgpt.com/docs/cli.md ; local installer script. A desktop-app-bundled CLI also exists (see below) | Script only: `irm https://antigravity.google/cli/install.ps1 \| iex` (Windows), `curl -fsSL https://antigravity.google/cli/install.sh \| bash` (macOS/Linux); binary at `%LOCALAPPDATA%\agy\bin` or `~/.local/bin`. No npm/winget/brew named. `agy install --dir --skip-path --skip-aliases` configures PATH/profile. VERIFIED (repo notes; troubleshooting page; local help) |
| Detect which install | `claude doctor` line "Config install method: native" (local); also path of `claude` on PATH (local: `~/.local/bin/claude`); `npm ls -g` and `winget list` for the others. VERIFIED (local) | `codex doctor --json` check `installation`, field `install context` (local: "standalone (windows, package ...\.codex\packages\standalone\releases\0.162.1-...)") plus `PATH codex #N`; and `updates.status.update action` ("standalone installer"). Local `codex` resolves to `AppData\Local\Programs\OpenAI\Codex\bin` which is a link to `~/.codex/packages/standalone/current/bin`. The `ChatGPT` MSIX desktop app is a separate install (`winget list` shows `MSIX\OpenAI.Codex_26.1007...`). VERIFIED (local) | Path only (`%LOCALAPPDATA%\agy\bin\agy.exe`); only one official method, so install method is effectively fixed. `agy update` reports status in `~/.gemini/antigravity-cli/updater/update_status.json` (local: "Already on the latest version."). UNVERIFIED that other methods exist |
| Auto-update | Native: background auto-update on by default, channel `latest` (default) or `stable` (`autoUpdatesChannel`); `DISABLE_AUTOUPDATER=1` stops background only, `DISABLE_UPDATES` blocks all paths; `minimumVersion` floor. Brew/WinGet/apt/dnf/apk/npm-with-unwritable-dir do not auto-update; `CLAUDE_CODE_PACKAGE_MANAGER_AUTO_UPDATE=1` makes Claude run brew/winget upgrade (WinGet may fail while running because Windows locks the exe). VERIFIED setup page | Startup check controlled by `check_for_update_on_startup` (default true locally); `in_app_updates` feature flag true; standalone install has an `auto-update-version` marker and a daemon updater pid file (`app-server-daemon/daemon-updater.pid`). Cached state `~/.codex/version.json` (`latest_version`, `last_checked_at`, `dismissed_version`). Exact auto-apply behaviour not documented. VERIFIED (local files/flags) / UNVERIFIED (behaviour) | Native static self-updater in the background, 15-minute debounce marker `last_check.timestamp`, advisory `update.lock` in `~/.gemini/antigravity-cli/updater/`; disable with `AGY_CLI_DISABLE_AUTO_UPDATE=true`; updates fail without write permission to the install dir. VERIFIED https://antigravity.google/docs/cli/troubleshooting |
| Manual update command | `claude update` (alias `upgrade`): reports "Successfully updated from X to version Y" or "Claude Code is up to date (X)"; Brew/WinGet/apk installs print "Claude is up to date!" and need the package manager (`winget upgrade Anthropic.ClaudeCode`, `brew upgrade claude-code`); npm: `npm install -g @anthropic-ai/claude-code@latest` (not `npm update -g`). `claude install [stable\|latest\|version]` reinstalls the native build. VERIFIED setup page; local help | `codex update` ("Check for and apply a Codex CLI update when the installed release supports self-update"; debug builds only print a message). Installer/npm: rerun the same command; brew: `brew upgrade --cask codex`. How `codex update` picks a route for npm vs standalone is not documented. VERIFIED docs; routing UNVERIFIED | `agy update` exists (no documented flags; `-h` errors with "flags provided but not defined: -h"). Docs pages I read do not describe it; behaviour inferred from `update_status.json`. Reinstall via the install script also works (inference). `agy changelog` prints release notes. VERIFIED (local) / UNVERIFIED (semantics) |
| Run non-interactively from a hosted terminal? | Yes: `claude update` takes no prompt and background update needs none. Caveats: replacing a running `claude.exe` on Windows (local `~\.local\bin` has `claude.exe.old.<ts>`, so the native updater renames the old binary, VERIFIED locally); WinGet upgrade can fail while it runs. Best done when no Claude tab is running. | `codex update` and the installer with `CODEX_NON_INTERACTIVE=1` look non-interactive (the installer's yes/no prompt returns false when input is redirected, VERIFIED in script). Not tested here because it would modify this machine. UNVERIFIED for `codex update` itself | `agy update` takes no documented prompt; not run here. UNVERIFIED |
| Check latest without updating | `https://downloads.claude.ai/claude-code-releases/latest` returns plain `2.1.301`; `.../stable` returns `2.1.292` (queried; matches setup page's channel model; endpoint is not documented, so UNVERIFIED as a contract). `npm view @anthropic-ai/claude-code dist-tags` gives `stable`, `latest`, `next` (queried: stable 2.1.292, latest 2.1.301). `claude doctor` shows the last update attempt. | `codex doctor --json` -> `updates.status` (`latest version` 0.163.0, `latest version status` "newer version is available", `update action`, `desktop latest build`); `~/.codex/version.json` cache; `npm view @openai/codex dist-tags.latest` (0.163.0); GitHub `gh api repos/openai/codex/releases/latest` (`rust-v0.163.0`). All queried. The installer script accepts `latest` or an exact `x.y.z`. | No documented version query. Options: run `agy changelog` (prints notes; locally the top entry was 1.4.0 while installed is 1.3.3, which suggests it shows the newest notes, UNVERIFIED), read `updater/update_status.json` after `agy update`. The install script URL returns 200 but I did not parse a version from it |
| Marks | VERIFIED unless noted | VERIFIED unless noted | mostly UNVERIFIED beyond the troubleshooting page |

Sources: https://code.claude.com/docs/en/setup ; https://learn.chatgpt.com/docs/cli.md ; https://learn.chatgpt.com/docs/developer-commands.md?surface=cli ; https://learn.chatgpt.com/docs/enterprise/manage-app-updates.md (desktop app, not read) ; https://antigravity.google/docs/cli/troubleshooting ; https://antigravity.google/docs/install?tab=cli.

Side note on Codex: this machine has two Codex installs, the standalone CLI (`~/.codex/packages/standalone`) and the desktop app (MSIX `OpenAI.Codex`, doctor reports "desktop update status: available"). A Marshell update button should key off `codex doctor --json` `installation`/`updates.status`, not assume npm.

---

## What Marshell can and cannot do

Can:
1. Host each CLI's own login in a pty (`claude auth login`, `codex login` / `--device-auth`, first `agy` launch) and leave the credential files and keyring alone. Claude's terms explicitly allow hosting the unmodified binary; Codex docs do not forbid it; Google's text is silent (treat as user-initiated plain terminal use).
2. Show "who is signed in" for Claude with `claude auth status` (email, `subscriptionType`, `authMethod`, `orgName`), for Codex with `codex login status` (text) plus `codex doctor --json` `auth.credentials` (auth mode, storage; no email), and for agy only from a running session's statusLine JSON (`email`, `plan_tier`, `quota`).
3. Treat sign-in state as "unknown/needs login" when `claude auth status` exits 1, `codex login status` fails, or agy prints "authentication required"; offer the CLI's own login.
4. Offer API-key paths only through each CLI's own mechanism (env vars, `codex login --with-api-key` fed from the user's own secret store, agy `GEMINI_API_KEY` plus `modelProvider`), never by moving subscription tokens.
5. List Codex cloud tasks via `codex cloud list --json` (experimental, ChatGPT sign-in); hand Claude web sessions to the user via `claude --teleport` in a tab; show only link-outs (claude.ai/code, chatgpt.com/codex) otherwise.
6. List local sessions through each CLI's own resume flows; read JSONL/sqlite transcripts only as a best-effort, version-gated extra (Claude JSONL "internal", Codex moving to sqlite, agy internal sqlite).
7. Run `claude doctor` (text), `codex doctor --json`; surface their output. For agy, build a Marshell-side check (binary found, `--version`, settings readable, keyring errors in `log/cli-*.log`).
8. Read versions (`--version` for all three), detect Claude install type from `claude doctor` and Codex from `doctor --json` `installation`, check latest via `codex doctor --json`, `npm view`, or Claude's `latest`/`stable` version URLs (undocumented but live), and run `claude update` / `codex update` / `agy update` in a tab when no session of that CLI is running.
9. Read and, through the config service with preview/backup/undo, write each CLI's own files: `~/.claude/settings.json` and `.mcp.json`, `~/.codex/config.toml`, `~/.gemini/antigravity-cli/settings.json` and `~/.gemini/config/mcp_config.json`. Translate MCP stdio entries between the three formats on explicit request; use `#:schema` / `$schema` URLs for validation of Claude and Codex.
10. Use skills as the portable extension unit: project `.agents/skills` for Codex and agy, `.claude/skills` for Claude (copy or symlink with user consent).

Cannot (or must not):
1. Collect, store, proxy or reuse Claude.ai OAuth tokens or `setup-token` output, offer a "Sign in with Claude" button, or resell/route subscription usage (Claude terms).
2. Read `~/.codex/auth.json` or the keyring entries to power its own features, or call OpenAI APIs on the user's plan, unless Marshell becomes an approved Sign in With ChatGPT client (open-source or partner programme; commercial apps need the interest form; UNVERIFIED approval status for Marshell).
3. Use agy credentials outside the `agy` binary or build automation that wraps `agy` for other agents/rotates accounts (Antigravity terms and FAQ; consumer plan risk of suspension).
4. Know when a login will really expire for any CLI; only react to an expired/failed state, or show Claude's 3-day startup warning if it appears in the pty output.
5. List claude.ai chat history, ChatGPT history, Claude cloud sessions, or Antigravity cloud conversations programmatically for a personal account (no public API; Claude Compliance API is Enterprise admin only).
6. Rely on a JSON `agy` status/doctor command (none exist) or on `claude doctor` JSON (none).
7. Share hooks or plugins across CLIs; manifests and marketplaces are per-CLI. Hooks written for Codex need the user to trust them in `/hooks`; Marshell must not automate that.
8. Treat Claude Desktop settings (`claude_desktop_config.json`) as Claude Code settings or write to them from the Claude Code settings screen.
9. Auto-update unattended while a session of that CLI is open (Windows locks the executable; WinGet upgrade can fail), or assume an npm install for Codex (the local one is standalone, plus a separate MSIX desktop app).

Open items I could not close: Codex plugin manifest name and location (page gives none); exact `agy update` semantics and flags; `codex cloud list --json` output shape (no tasks to list); whether Claude reads `.agents/skills`; whether Codex reads `~/.codex/skills` locally; OpenAI help page and DevDay recap (403).
