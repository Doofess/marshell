# S11: Agent CLIs inside WSL

Run 2026-10-31 on the dev box (Windows 11 Pro 26300), against `phase-6j1` under `pnpm tauri dev` with a scratch `MARSHELL_HOME`.
- **Distro:** Ubuntu 26.04.1 LTS on WSL 2, kernel 6.18.54.1, NAT networking. Before the spike it had no claude, codex, agy or Linux node.
- **Install:** the CLIs went into the tmpfs scratch folder `/tmp/s11`, with `HOME=/tmp/s11/home` (slice 6j1 decision 3). Each came from its official release, checked against the manifest digest:
  - Claude Code 2.1.316;
  - Codex 0.166.0, as the musl `codex-package`;
  - Antigravity CLI 1.4.12.
- **Hook entries:** the ones Marshell wrote for the Windows CLIs, copied into the Linux homes with the bridge at its `/mnt/c/...` path.
- **Driver:** a Marshell "Ubuntu" tab, driven over CDP. The user signed Claude Code and agy in by browser.

| Check | Pass criterion | Result |
|---|---|---|
| The bridge runs from Linux | A Linux CLI can run the Windows `marshell-hook.exe` and reach the core | **Pass.** Interop runs it in 28–37 ms with stdin piped in, inside the bridge's 400 ms budget. The exe inherits the Windows environment `wsl.exe` was started with, which includes `MARSHELL_TAB_ID` and `MARSHELL_HOME`. It reads the token from the Windows `endpoint.json` and posts to Windows loopback. No token or port enters Linux. Its cwd is the UNC form of the Linux folder, for example `\\wsl.localhost\Ubuntu\tmp`. |
| Claude Code approvals | A permission request shows a card, and the card's answer reaches Claude | **Pass.** In manual mode, "Create the file notes.txt…" showed "Needs you · 1" with the card "Create /tmp/s11/proj/notes.txt" within about 2 s. Allow once on the card let Claude write the file. |
| Codex approvals | The same for Codex | **Pass.** It needed the packaged Codex and `--no-daemon`. `curl -sI https://example.com` failed in the bwrap sandbox, and Codex asked to escalate. Marshell showed "Run a command / Reaches example.com on the network". Deny on the card made Codex print "Blocked by hook / Denied in Marshell." |
| Antigravity hooks | agy runs Marshell's hook entries | **Pass.** agy reads `$HOME/.gemini/config/hooks.json` and `antigravity-cli/settings.json`. A logging wrapper saw every entry run: statusline every ~300 ms, PreInvocation, PreToolUse, PostToolUse, PostInvocation and Stop. |
| Antigravity approvals | A card for a risky agy call | **Fail.** No card. `hook.rs` answers agy's PreToolUse with 204 unless `effective_agent(tab)` is `antigravity`, which a WSL tab never is. |
| Live status on the row | The row follows the agent | **Fail** for all three CLIs. The row read "WSL, Idle" throughout. `toRow.ts` shows live status only when `info.agent` or the foreground names an agent, and the core can't see Linux processes. |
| References | A prompt naming an alias gets the note | **Fail by design today.** It wasn't run: `note_tab` needs `effective_agent(tab)` to equal the agent. The cwd it would resolve in is a Linux path. |
| History | Agent sessions in WSL reach History | **Fail.** The index had 0 sessions after a turn in each of the three CLIs. The hook bodies carry Linux transcript paths. |

## Findings

1. **One cause blocks most features: the core never learns which agent runs in a WSL tab.**
   - `Registry::effective_agent` uses three sources: the launched agent, `displaced_by`, and the foreground watcher. The watcher reads Windows processes, and from Windows it sees only `wsl.exe`.
   - Live status, agy cards, references notes and the "another agent's hooks" guard all key on it.
   - Claude and Codex cards work only because `hold_permission` needs nothing but an open tab.
2. **Paths are Linux paths.**
   - Hook bodies carry Linux paths in `cwd` and `transcript_path`, for example `/tmp/s11/proj`.
   - The Windows core reaches such a path as `\\wsl.localhost\<distro>\...`, or for `/mnt/<drive>/...` as `<drive>:\...`.
   - References and History need that translation, and the core needs to know the tab's distro.
3. **Setup is Windows-only.**
   - Marshell writes hook entries only into the Windows homes.
   - A CLI in a distro reads its own Linux home: `~/.claude/settings.json`, `~/.codex/hooks.json` (then trust in `/hooks`), and agy's `~/.gemini/config/hooks.json` and `~/.gemini/antigravity-cli/settings.json`.
   - The entries need the bridge as a Linux path (`/mnt/c/...`, from the distro's automount root).
   - Writing them is a config write inside the distro, under the same preview, backup and managed-file rules.
4. **The version gate asks the wrong CLI.** `fold_live_with` checks the Windows CLI's version, not the one running in the distro.
5. **Sign-in inside a bare distro.**
   - Ubuntu 26.04 has no `wslview`/`xdg-open` handler, so Claude Code and agy can't open a browser.
   - Claude Code's fallback URL is about 430 characters and hard-wrapped over three rows. Clicking or copying it gave "invalid OAuth request".
   - A `BROWSER` script running `rundll32.exe url.dll,FileProtocolHandler "$1"` through interop worked.
   - agy's one-row "Click here to authenticate" link (OSC 8) also works.
   - agy on Linux ignores the Windows `oauth_creds.json` and `google_accounts.json` copied into its home (its docs say Linux uses the keyring).
6. **Codex from a bare binary.**
   - The `codex-x86_64-unknown-linux-musl` binary alone refuses its daemon ("this CLI has no complete local package"). Its shell tool fails without `codex-code-mode-host`.
   - The release's `codex-package` works.
   - With `CODEX_HOME` under `/tmp`, Codex refuses to create its helper binaries unless `TMPDIR` points elsewhere. A normal install isn't under `/tmp`.
7. **Two key conflicts, not specific to WSL.**
   - Codex's "f2 to view" can't be reached: F2 renames the Marshell row.
   - Codex reads an Enter right after a fast typed burst as a newline.
