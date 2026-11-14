# Slice 9a, alpha builds installed with winget: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A friend types `winget install Doofess.Marshell`, gets a working alpha with no administrator prompt, updates it with `winget upgrade`, and `winget uninstall` leaves no trace in any CLI config.

**Architecture:** A tag `v0.1.0-alpha.N` starts `.github/workflows/release.yml`. It stamps the version, builds the per-user NSIS installer with the three helper programs in it, smoke-tests the installer on the runner, and publishes a GitHub pre-release with the installer, its checksum and a winget manifest folder. A second job installs that release with winget on a fresh runner. A third submits the manifest to `microsoft/winget-pkgs` with `wingetcreate`.
- **The installer's own work** is in `src-tauri/windows/hooks.nsh`: it writes the quiet uninstall command winget runs, and the uninstaller runs `marshell uninstall --yes` before it removes files.
- **The release scripts** in `scripts/release/` are pure functions with a thin command line, tested with Vitest.
- **The smoke test** is one PowerShell script with three ways to install (the setup program, a manifest folder, the community repository).

**Tech Stack:** Tauri 2 CLI 2.12.1 (NSIS bundler), GitHub Actions on `windows-latest`, Node 24 scripts (`.mjs`, no new package), PowerShell 7, Rust for one example program, winget and Microsoft's `wingetcreate` 1.28.1.0.

**Spec:** `docs/PLAN.md` §9, phase 9, "Slice 9a decisions (2026-11-14)" 1 to 14. Evidence: `docs/research/2026-11-14-winget-alpha.md`.

## Global Constraints

- Branch `phase-9a`, cut from `origin/main` at `57e077c7`, in the main checkout. Ledger: `.superpowers/sdd/2026-11-14-phase-9a-alpha-winget/progress.md`. Tasks 3 and 4 go to Sonnet workers; the lead does the rest (decision 13).
- The id is `Doofess.Marshell`, the publisher `Doofess`, the package name `Marshell`. Versions read `0.1.0-alpha.N` with N from 1; the tag is `v` plus the version. A dry run builds `0.1.0-alpha.0`.
- The installer is the per-user NSIS one for x64, unsigned. Its file is `Marshell_<version>_x64-setup.exe`, and its address is `https://github.com/Doofess/marshell/releases/download/v<version>/Marshell_<version>_x64-setup.exe`.
- Uninstall keeps `~/.marshell` except `bin` (decision 9). An uninstall that is part of an update keeps the hooks.
- No new crate or package. No change to the CSP, the window settings or the capabilities. `tauri.conf.json` gains only `bundle.publisher`. `ci.yml` is not touched (decision 12).
- The secret is `WINGET_TOKEN`. It reaches `wingetcreate` through the `WINGET_CREATE_GITHUB_TOKEN` environment variable, never a command line. No job that runs on a pull request reads it.
- The smoke test runs on GitHub's runners only. It is not run on the development PC without the user's word: it installs Marshell for the signed-in user, and a running Marshell would take the installed copy's start.
- The smoke test never reads `run/endpoint.json` (the core's token), never stops a process by name and never deletes a folder itself. It stops only the PIDs it started.
- One Rust build at a time on the development PC, with `-j 4`.
- Tests fail first, except pins of existing behaviour. Scripts: `pnpm test scripts/release`. Before the PR, what CI runs (`marshell-ship`).
- Commit messages read `type(scope): what changed, as a sentence.` and end with the session's trailers.

## Review Focus

1. **A newer installer run by hand over an install, with "uninstall before installing" chosen:** the old uninstaller may run without `/UPDATE`, and the unhook would then take the friend's hooks out on an update. A person expects an update to keep the hooks. Task 1 reads the generated installer script to settle which flows pass `/UPDATE`; Task 2 guards what the script lets it tell apart; what's left goes in the release notes ("update with winget") and Known limits.
2. **`winget upgrade` while Marshell is open:** the installer runs silently over the install. A person expects the app to close, the files to be replaced and the hooks to stay. Task 4's smoke test installs a second time over a running app and asserts all three.
3. **Uninstall when one CLI's file can't be changed** (it doesn't parse): the uninstall must still finish, leave that file byte for byte, take the other CLIs' hooks out and keep `~/.marshell/bin`. Task 4's second cycle pins it.
4. **A tag that isn't an alpha version** (`v0.1.0`, `v1`, `v0.1.0-alpha.0`, `v0.1.0-alpha.1-x`): the workflow refuses before it builds or publishes anything. Task 3's tests pin it.
5. **Uninstall started from Add/Remove Programs:** it has Explorer's environment, so a `CLAUDE_CONFIG_DIR`, `CODEX_HOME` or `COPILOT_HOME` set only in a shell isn't seen, and hooks in that folder stay. No test can cover it on a runner. Task 5 says so in the release notes and `docs/RELEASING.md`, and the PR lists it under Known limits.

---

### Task 1: The installer carries the helper programs (lead)

**Files:**
- Create: `src-tauri/tauri.release.conf.json`, `scripts/release/stage-programs.mjs`
- Modify: `src-tauri/tauri.conf.json` (`bundle.publisher`), `.gitignore` (`/src-tauri/staged/`)
- Create: `docs/research/2026-11-14-alpha-installer.md`

**Interfaces:**
- Produces:
  - `node scripts/release/stage-programs.mjs`: copies `target/release/marshell-hook.exe`, `marshell.exe` and `marshell-host.exe` into `src-tauri/staged/`. A missing one is an error that names it, exit 1.
  - `src-tauri/tauri.release.conf.json`, passed with `--config`: sets `build.beforeBuildCommand` to `pnpm build:hook:release && node scripts/release/stage-programs.mjs && pnpm build`, and adds `bundle.resources` entries `staged/<name>.exe` → `<name>.exe` for the three programs. The base config stays as it is for `tauri dev` and CI, where `target/release` may be empty.
  - The build command every later task uses: `pnpm tauri build --bundles nsis --config src-tauri/tauri.release.conf.json`. Output: `target/release/bundle/nsis/Marshell_<version>_x64-setup.exe`; the generated script is `target/release/nsis/x64/installer.nsi`.

- [ ] **Step 1:** Add `"publisher": "Doofess"` to `bundle` in `tauri.conf.json`, write the release config and the staging script, and ignore `/src-tauri/staged/`.
- [ ] **Step 2: Build the installer locally** with the command above. Expected: the setup file exists, and the build log lists the three programs among the resources.
- [ ] **Step 3: Read the generated `installer.nsi`** and write the findings to the research note, each with the script's line:
  - the files the installer copies (the three programs, `conpty.dll`, `OpenConsole.exe`, the licence) and that `--config` merged `bundle.resources` with the Windows config's entries instead of replacing them;
  - the uninstall key's name and its `Publisher` (expected `Doofess`), and whether `QuietUninstallString` is written (expected: no);
  - the hook macros' names and where each is called, and whether `${UNINSTKEY}` and `$UpdateMode` are in scope there;
  - which flows run the old uninstaller, and which of them pass `/UPDATE`: a silent install over an install, a passive one, and the reinstall page's choices (Review Focus 1);
  - what the uninstaller does to a running app when silent, and what a silent uninstall does with the "Delete application data" box.
- [ ] **Step 4: Commit** `build(installer): the installer carries marshell-hook, marshell and marshell-host, and names Doofess as its publisher.`

### Task 2: The installer's quiet uninstall and the unhook (lead)

**Files:**
- Modify: `src-tauri/windows/hooks.nsh`

**Interfaces:**
- Consumes: Task 1's findings (macro names, `${UNINSTKEY}`, `$UpdateMode`).
- Produces, relied on by Task 4's assertions:
  - After install, the uninstall key holds `QuietUninstallString` = `"<install folder>\uninstall.exe" /S`.
  - The uninstaller runs `"$INSTDIR\marshell.exe" uninstall --yes` through `nsExec::ExecToLog` before any file is removed, when `$UpdateMode <> 1` and the program exists. Its exit code is read and ignored: the uninstall goes on (decision 14).

- [ ] **Step 1:** Add `NSIS_HOOK_POSTINSTALL` (the registry value) and `NSIS_HOOK_PREUNINSTALL` (the unhook), each with a comment that says why and cites decision 14. Keep `NSIS_HOOK_POSTUNINSTALL` as it is.
- [ ] **Step 2:** If Task 1 found a flow that runs the old uninstaller without `/UPDATE` during an install, and the script gives the hook a way to tell (a variable, a command-line switch the hook can read with `${GetOptions}`), skip the unhook there too. If it gives none, write that in the research note for Task 5's release notes.
- [ ] **Step 3: Rebuild the installer** (Task 1's command). Expected: `makensis` compiles, and the generated script shows both macros expanded.
- [ ] **Step 4: Commit** `feat(installer): uninstalling takes Marshell's hooks out of every CLI, and winget gets a quiet uninstall command.`

### Task 3: The version stamp and the winget manifest (worker)

**Files:**
- Create: `scripts/release/version.mjs`, `scripts/release/set-version.mjs`, `scripts/release/winget-manifest.mjs`
- Test: `scripts/release/version.test.ts`, `scripts/release/winget-manifest.test.ts`

**Interfaces:**
- Produces:
  - `version.mjs`:
    - `export const DRY_RUN_VERSION = "0.1.0-alpha.0";`
    - `export function versionFromTag(tag: string): string`. Accepts only `/^v(\d+\.\d+\.\d+-alpha\.[1-9]\d*)$/` and returns the capture; anything else throws an `Error` whose message names the tag and the expected form `v0.1.0-alpha.N`.
    - `export function stamp(files: { cargoToml: string; tauriConf: string; packageJson: string }, version: string): { cargoToml: string; tauriConf: string; packageJson: string }`. Text edits that keep every other byte: the `version = "..."` line under `[workspace.package]` in `Cargo.toml`, and the top-level `"version": "..."` in the two JSON files. It throws when a file doesn't have exactly one such line.
    - `export function installerName(version: string): string` → `Marshell_<version>_x64-setup.exe`, and `export function installerUrl(version: string): string` → the address in Global Constraints.
  - `set-version.mjs --tag <tag>` or `--dry-run`: stamps `Cargo.toml`, `src-tauri/tauri.conf.json` and `package.json` in place and prints the version as the only line on stdout. A bad tag prints the error to stderr, exit 1, and changes nothing.
  - `winget-manifest.mjs`:
    - `export function manifests(input: { version: string; sha256: string; releaseDate: string }): Record<string, string>`. Keys are exactly `Doofess.Marshell.yaml`, `Doofess.Marshell.installer.yaml` and `Doofess.Marshell.locale.en-US.yaml`. It throws unless `sha256` is 64 upper-case hex characters and `releaseDate` is `YYYY-MM-DD`.
    - Command line: `--version <v> --installer <setup file> --out <folder> [--date YYYY-MM-DD]`. It hashes the file, writes the three files into the folder (created if missing) and prints the hash. The date defaults to today in UTC.
- The manifest's content, `ManifestVersion: 1.10.0` in all three, each starting with its `# yaml-language-server: $schema=https://aka.ms/winget-manifest.<type>.1.10.0.schema.json` line:
  - version: `PackageIdentifier: Doofess.Marshell`, `PackageVersion`, `DefaultLocale: en-US`, `ManifestType: version`.
  - installer: `PackageIdentifier`, `PackageVersion`, `InstallerType: nullsoft`, `Scope: user`, `InstallModes` (`interactive`, `silent`), `UpgradeBehavior: install`, `ReleaseDate`, one entry in `Installers` with `Architecture: x64`, `InstallerUrl`, `InstallerSha256` and `ProductCode: Marshell` (the uninstall key's name), `ManifestType: installer`.
  - locale: `PackageIdentifier`, `PackageVersion`, `PackageLocale: en-US`, `Publisher: Doofess`, `PublisherUrl: https://github.com/Doofess`, `PublisherSupportUrl: https://github.com/Doofess/marshell/issues`, `PackageName: Marshell`, `PackageUrl: https://github.com/Doofess/marshell`, `License: MIT`, `LicenseUrl: https://github.com/Doofess/marshell/blob/main/LICENSE`, `ShortDescription: A desktop terminal for running AI coding agents side by side.`, `Tags` (`terminal`, `ai`, `claude-code`, `codex`), `ReleaseNotesUrl: https://github.com/Doofess/marshell/releases/tag/v<version>`, `ManifestType: defaultLocale`.

- [ ] **Step 1: Write the failing tests.**
  - `version.test.ts`:
    - "an alpha tag gives its version": `versionFromTag("v0.1.0-alpha.1")` → `"0.1.0-alpha.1"`, and `"v0.2.3-alpha.12"` → `"0.2.3-alpha.12"`.
    - "anything else is refused, naming the tag" (Review Focus 4): `v0.1.0`, `v1`, `0.1.0-alpha.1`, `v0.1.0-alpha.0`, `v0.1.0-alpha.1-x`, `v0.1.0-beta.1` and `""` each throw, and the message contains the tag and `v0.1.0-alpha.N`.
    - "the stamp changes the three version lines and nothing else": run `stamp` over the repo's real three files with `0.1.0-alpha.7`; each result differs from its input in one line, which holds `0.1.0-alpha.7`; a dependency's `version = "1"` in `Cargo.toml` is unchanged.
    - "a file without its version line is refused": `stamp` with `cargoToml: "[workspace]\n"` throws.
    - "the installer's name and address": `installerName("0.1.0-alpha.1")` → `Marshell_0.1.0-alpha.1_x64-setup.exe`; `installerUrl` → the full address.
  - `winget-manifest.test.ts`, with `sha = "AB".repeat(32)` and `releaseDate: "2026-11-15"`:
    - "three files, named for the id": the keys.
    - "the installer manifest is a per-user Nullsoft one": contains `InstallerType: nullsoft`, `Scope: user`, `ProductCode: Marshell`, `InstallerUrl: ` plus `installerUrl(version)`, `InstallerSha256: ` plus the hash, `ReleaseDate: 2026-11-15`.
    - "the locale manifest matches Add/Remove Programs": contains `Publisher: Doofess` and `PackageName: Marshell`.
    - "every file names the id, the version and the manifest version".
    - "a hash that isn't 64 upper-case hex characters, or a date that isn't a date, is refused": lower case, 63 characters, `2026-11-5`.
- [ ] **Step 2: Run, expect FAIL** (modules missing): `pnpm test scripts/release`
- [ ] **Step 3: Implement** the three modules. YAML is written as plain strings; no package is added.
- [ ] **Step 4: Run, expect PASS**: `pnpm test scripts/release`, then `pnpm build` (the type check covers the tests).
- [ ] **Step 5: Validate a generated folder with the local winget:** `node scripts/release/winget-manifest.mjs --version 0.1.0-alpha.1 --installer package.json --out <scratch folder>`, then `winget validate --manifest <scratch folder>`. Expected: "Manifest validation succeeded" (a warning is reported, not fixed by guessing). Report the full output. Don't run `set-version.mjs` against the repo outside a test; if you do, restore the three files with `git checkout`.
- [ ] **Step 6: Commit** `feat(release): a tag gives the version stamped into the build, and a script writes the winget manifest.`

### Task 4: The smoke test (worker, `effort: high`)

**Files:**
- Create: `crates/cli/examples/smoke-fixture.rs`, `scripts/release/smoke.ps1`

**Interfaces:**
- Consumes: Task 2's registry value and unhook; `marshell list --json` (exit 0 when the app answers, 1 when it is closed) and `marshell --version` (`marshell <version>`).
- Produces:
  - `smoke-fixture <marshell home>`: puts Marshell's hooks into Claude Code's and Codex's files with the app's own plans (`claude::hooks::install_plan` with `status` and `statusline`, `codex::hooks::install_plan` with `status`), each previewed and applied through `ConfigService::new(home)`, as `installed()` and `install_codex()` do in `crates/cli/src/uninstall.rs`'s tests. It finds the files as the CLI does: `settings_path` and `hooks_path` with the process environment and `USERPROFILE`. Claude Code's `settings.json` must exist already; Codex's `hooks.json` may be missing, and its folder must exist. It changes nothing else. It prints each file it changed; any failure prints why, exit 1. Built with `cargo build --release -p marshell-cli --example smoke-fixture`.
  - `smoke.ps1`, PowerShell 7, exit 0 when every check passed and 1 otherwise, with one `PASS`/`FAIL`/`SKIP` line per check and a count at the end. Every failed check is reported, not only the first.
    - `-Version <v>` (required), `-Fixture <smoke-fixture.exe>` (optional: without it the hook checks are `SKIP`), `-Scratch <folder>` (optional; default a new folder under `$env:TEMP`; it must not exist yet and must be under the user's profile, because the config guard allows only that).
    - One of `-Installer <setup file>`, `-WingetManifest <folder>` or `-WingetId Doofess.Marshell`.
- What the script does. It sets `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `COPILOT_HOME` to folders under the scratch folder for itself and everything it starts.
  - **Before anything:** it refuses to run (exit 1, nothing changed) when the uninstall key `HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Marshell` exists or a `marshell-app` process is running.
  - **Cycle A, the usual life:**
    1. Install. `-Installer`: the setup file with `/S`, waited for. `-WingetManifest`: `winget install --manifest <folder> --silent --accept-package-agreements --accept-source-agreements --disable-interactivity`. `-WingetId`: `winget install --id <id> --exact --version <v> --source winget` with the same switches.
    2. Check the install: the key exists under HKCU and not under HKLM; `DisplayName` is `Marshell`, `Publisher` is `Doofess`, `DisplayVersion` is the version; `QuietUninstallString` is there; the install folder is under `$env:LOCALAPPDATA` and holds `marshell-app.exe`, `marshell.exe`, `marshell-hook.exe`, `marshell-host.exe`, `conpty.dll`, `OpenConsole.exe` and `uninstall.exe`; the installed `marshell.exe --version` prints `marshell <version>`.
    3. Start `marshell-app.exe` and keep its PID. Within 90 s the installed `marshell.exe list --json` exits 0, and `<home>\bin` holds `marshell-hook.exe` and `marshell.exe`.
    4. Write a known Claude Code `settings.json` (with a `statusLine` of its own), create Codex's folder with no `hooks.json`, and run the fixture. Check that both files now name `marshell-hook`.
    5. Install again over the running app, the way `winget upgrade` does (`-Installer` only; the winget modes `SKIP`). Check that the installer exits 0, the app's PID has ended, and Claude Code's file still names `marshell-hook` (Review Focus 2).
    6. Start the app again, wait for `list` as in 3.
    7. Uninstall with the app open. `-Installer`: run the key's `QuietUninstallString`. The winget modes: `winget uninstall --id Doofess.Marshell --exact --silent --disable-interactivity` (`--manifest <folder>` in manifest mode if the id isn't found). The uninstaller hands over to a copy of itself and returns early, so wait up to 120 s for the key to go.
    8. Check the uninstall: the key is gone; the app's PID has ended; the install folder holds no file; Claude Code's `settings.json` is byte for byte the known one; Codex's `hooks.json` is gone or doesn't name `marshell-hook`; `<home>\bin` is gone; `<home>\snapshots` is still there (decision 9); `HKCU:\Software\Classes\marshell` doesn't point into the install folder; the Start menu has no `Marshell.lnk`.
  - **Cycle B, a file that can't be changed** (Review Focus 3; needs `-Fixture`, else `SKIP`): install, start the app, wait for `list`, stop the app by its PID, write the known `settings.json`, run the fixture, then overwrite Codex's `hooks.json` with `{ "hooks": [oops` and a newline. Uninstall as in step 7. Check: the key is gone; Claude Code's file is the known one; Codex's file is byte for byte the broken one; `<home>\bin\marshell-hook.exe` is still there.
  - **At the end** it prints the scratch folder's path and leaves it. If a check failed while the app runs, it stops the PIDs it started and no others.

- [ ] **Step 1: Write `smoke-fixture.rs`** and build it: `cargo build --release -p marshell-cli --example smoke-fixture -j 4`.
- [ ] **Step 2: Prove the fixture against the real unhook, in a scratch folder under `%TEMP%`** (a literal new folder; set `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `CODEX_HOME` there for these commands only): write a `settings.json`, run the fixture, check both files name `marshell-hook`, run `target/release/marshell.exe uninstall --yes` with the same environment, and check `settings.json` is back byte for byte. Report the commands and their output. This is the only local run; it installs nothing.
- [ ] **Step 3: Write `smoke.ps1`.** Don't run it: it installs the app, and it runs on GitHub's runner in Task 6. Check it parses: `pwsh -NoProfile -Command "$null = [scriptblock]::Create((Get-Content -Raw scripts/release/smoke.ps1)); 'parsed'"`. Check the refusal locally only if the uninstall key is absent and no `marshell-app` runs; otherwise report that it wasn't run.
- [ ] **Step 4:** `cargo fmt --all`, then `cargo clippy -p marshell-cli --all-targets -j 4 -- -D warnings`. Expected: clean.
- [ ] **Step 5: Commit** `test(release): a smoke test installs the alpha, starts it, updates over it and uninstalls it, and checks what each left behind.`

### Task 5: The release workflow, the notes and the docs (lead)

**Files:**
- Create: `.github/workflows/release.yml`, `scripts/release/notes.mjs`, `scripts/release/notes.test.ts`, `docs/RELEASING.md`
- Modify: `README.md` (the status line and an "Install" section), `AGENTS.md` (the project map's `scripts/` line and one line on the release workflow)

**Interfaces:**
- Consumes: Tasks 1 to 4's commands, as named in their Interfaces.
- Produces:
  - `notes.mjs`: `export function releaseNotes(input: { version: string; sha256: string }): string` (Markdown), and a command line `--version <v> --sha256 <hash>` that prints it. The notes say, in this order: what the build is and where to report problems; install with `winget install Doofess.Marshell`; the fallback (download `winget-manifest-<version>.zip`, `winget settings --enable LocalManifestFiles` once as administrator, `winget install --manifest <folder>`); the SmartScreen warning and how to get past it, and that no administrator rights are needed; update with `winget upgrade Doofess.Marshell` and not by running a newer installer over the old one, if Task 2 left that flow unguarded; uninstall, what it removes and that `%USERPROFILE%\.marshell` stays; the hooks left when a CLI's config folder is set only in a shell (Review Focus 5); the installer's SHA-256.
  - `release.yml`, `permissions: contents: read` at the top:
    - **Triggers:** a pushed tag `v*`; a pull request that touches `.github/workflows/release.yml`, `src-tauri/windows/**`, `src-tauri/tauri*.conf.json`, `src-tauri/build.rs`, `scripts/release/**`, `crates/cli/examples/**` or `crates/cli/src/uninstall.rs`; `workflow_dispatch` with `action` (`dry-run`, `check`, `submit`) and `tag`.
    - **`build`** (`windows-latest`; a tag, a pull request, or a `dry-run`): the setup steps of `ci.yml`; `set-version.mjs` (`--tag` for a tag, else `--dry-run`); Task 1's build command; the fixture's build; the manifest, the `.sha256` file and the notes; `winget validate` when the runner has winget; `smoke.ps1 -Installer`; then the installer, its `.sha256`, `winget-manifest-<version>.zip`, the notes and the fixture uploaded as one artifact.
    - **`publish`** (a tag only, after `build`, `contents: write`): `gh release create v<version> --prerelease --verify-tag --title "Marshell <version>" --notes-file` with the installer, the `.sha256` and the manifest zip.
    - **`winget-check`** (`windows-latest`; after `publish`, or `check` with a tag): makes winget available, runs `winget settings --enable LocalManifestFiles`, downloads the release's manifest zip and runs `smoke.ps1 -WingetManifest`. With `check` and a tag whose package is in the community repository it runs `-WingetId` instead. The fixture comes from the run's artifact, when there is one.
    - **`winget-submit`** (`windows-latest`; after `publish`, or `submit` with a tag): without `WINGET_TOKEN` it writes a notice that says how to submit later and ends green. With it: downloads the release's manifest zip and `wingetcreate.exe` 1.28.1.0 from `microsoft/winget-create`'s release, and runs `wingetcreate submit --prtitle "New version: Doofess.Marshell version <version>" --no-open <folder>`.
  - `docs/RELEASING.md`: how to cut an alpha (the tag), what each job does, how to make the token and where it goes, how to submit a release that went out before the token, the fallback for a refused submission, and how to bump the pinned `wingetcreate`.

- [ ] **Step 1: Write the failing test** `notes.test.ts`: "the notes carry the commands, the warning and the checksum": the text contains `winget install Doofess.Marshell`, `winget upgrade Doofess.Marshell`, `winget uninstall Doofess.Marshell`, `winget settings --enable LocalManifestFiles`, `SmartScreen`, `%USERPROFILE%\.marshell`, `winget-manifest-0.1.0-alpha.1.zip`, `Marshell_0.1.0-alpha.1_x64-setup.exe` and the hash. Run, expect FAIL: `pnpm test scripts/release/notes.test.ts`.
- [ ] **Step 2: Write `notes.mjs`.** Run, expect PASS.
- [ ] **Step 3: Write `release.yml`.** Check each `if:` against the table of triggers above, and that no pull-request path reaches `publish`, `winget-submit` or the secret.
- [ ] **Step 4: Write `docs/RELEASING.md`, and update `README.md` and `AGENTS.md`.**
- [ ] **Step 5: Commit** in two parts: `ci(release): a tag builds the installer, smoke-tests it, publishes a pre-release and submits it to winget.` and `docs(release): how to cut an alpha, and how friends install one.`

### Task 6: The dry run on GitHub (lead)

- [ ] **Step 1:** Push `phase-9a` and open the pull request as a draft. The release workflow's dry run starts on it, beside `ci`.
- [ ] **Step 2:** Read the run: the build, `winget validate`, and every `PASS`/`FAIL`/`SKIP` line of the smoke test. A failure is fixed on the branch at its cause (systematic-debugging); a check is never weakened to pass. Each finding goes in `docs/research/2026-11-14-alpha-installer.md` with the run's number: what the runner has (winget, WebView2, a desktop for the app), what `QuietUninstallString` did, and how the update over a running app went.
- [ ] **Step 3: Commit** the note: `docs(research): what the alpha installer does on a clean runner.`

### Task 7: Review and ship (lead)

- [ ] **Step 1:** The whole-branch review by `marshell-reviewer` on Opus, with this plan's Review Focus, then one fix pass.
- [ ] **Step 2:** `marshell-ship`: what CI runs, the pull request's body (the dry run is its live check), a green CI and release dry run, the merge, and the main checkout fast-forwarded.

### Task 8: The first alpha (lead; decision 11)

- [ ] **Step 1:** On the merged `main`: `git tag v0.1.0-alpha.1`, `git push origin v0.1.0-alpha.1`. Watch `build`, `publish` and `winget-check`. Expected: the pre-release is public with its three files, and `winget-check` installed it from the manifest folder, started it and uninstalled it.
- [ ] **Step 2:** Tell the user the release is out and ask for the token: a classic token with `public_repo`, saved as the repo secret `WINGET_TOKEN`. Then run the workflow with `action: submit` and the tag. Expected: a pull request on `microsoft/winget-pkgs`; the user signs the contributor agreement when the bot asks. If `wingetcreate submit` refuses a new package, decision 5's hand submission stands and the user is told the command.
- [ ] **Step 3:** Record in the research note and PLAN §9 what happened: the release's address, `winget-check`'s result, the submission's pull request and what its validation said. When Microsoft accepts it, run `action: check` with the tag for the exit's `winget install Doofess.Marshell`. Docs pull request, merged.
- [ ] **Step 4:** Delete the ledger directory, and update the memory note on the next slice (6q-3b).
