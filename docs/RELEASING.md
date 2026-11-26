# Releasing an alpha

Alpha builds are for people who want to try Marshell before v1. They install with winget, cost nothing to publish and are not signed. The decisions are in `docs/PLAN.md` §9, "Slice 9a decisions"; the evidence is in `docs/research/2026-11-14-winget-alpha.md` and `docs/research/2026-11-14-alpha-installer.md`.

## Cut a release

1. `main` is green and holds what the release should carry.
2. Run a dry run on `main` and wait for it: `gh workflow run release.yml -f action=dry-run`. A pull request runs one only when it touches the installer or the release scripts, and a change elsewhere (the CLI's uninstall, an adapter's hooks) can break what the smoke test checks. A tag whose build fails publishes nothing, but its number is used up.
3. Decide whether the tag submits to winget. While the repository variable `WINGET_HOLD` is set, it doesn't (see [Holding the submission](#holding-the-submission)), so set or delete the variable before you push the tag. If the tag will submit, press **Sync fork** on the fork's page (`https://github.com/<owner>/winget-pkgs`) first. The workflow tries the sync itself, but GitHub refuses it whenever `microsoft/winget-pkgs` has changed one of its own workflow files since the fork's last sync, which happened within a week of alpha.1 (see [The fork](#the-fork)).
4. Tag it and push the tag. The tag is the version: `v0.1.0-alpha.N`, with N one more than the last release.

   ```sh
   git tag v0.1.0-alpha.2
   git push origin v0.1.0-alpha.2
   ```

5. Watch the `release` workflow. Nothing else is needed: the version is stamped into the build from the tag, and no file in the repo changes.

A tag of any other form stops the workflow before it builds.

## What the workflow does

`.github/workflows/release.yml` has four jobs.

| Job | When | What it does |
| --- | --- | --- |
| `build` | A tag, a pull request that touches the installer or the release scripts, or a manual `dry-run` | Stamps the version (`0.1.0-alpha.0` for a dry run), builds the per-user NSIS installer, writes the winget manifest, the checksum and the release notes, and runs the smoke test on the installer. |
| `publish` | A tag, after `build` | Creates the GitHub pre-release with the installer, its `.sha256` and `winget-manifest-<version>.zip`. |
| `winget-check` | After `publish`, or a manual `check` | On a fresh runner: installs the release with winget from its manifest folder, starts the app and uninstalls it with winget. |
| `winget-submit` | After `winget-check` passes, or a manual `submit` | Syncs the owner's fork with `microsoft/winget-pkgs`, then opens the pull request there with Microsoft's `wingetcreate`, titled "New package:" until the package has a folder there and "New version:" after. Without the token, or on a tag's run while `WINGET_HOLD` is set, it submits nothing, says so and ends green. When the fork can't be synced, it stops with what to do. |

The smoke test is `scripts/release/smoke.ps1`. It installs silently, checks the uninstall entry and the installed files, starts the app and asks it for its sessions, updates over the running app, uninstalls, and checks what is left: no hooks in the CLI files, no program files, and `~/.marshell` kept without its `bin` folder. It runs on GitHub's runners and nowhere else: each uninstall in it is a real one, which also takes Marshell's hooks out of `~/.gemini`, `~/.agents` and every WSL distro, and no variable moves those. The script refuses to start outside GitHub Actions (a VM that is thrown away afterwards can pass `-ThrowawayMachine`), and where Marshell is installed or running.

`scripts/release/hooks-check.ps1` tries the uninstaller's wait for program files that are still in use (`src-tauri/windows/hooks.nsh`) on stand-in files in the temp folder: a file that frees up is deleted, and a file that never does is given up on at the limit. It installs nothing, so it runs on a development PC once an installer has been built there (it uses the NSIS that Tauri downloads). No workflow runs it: run it by hand after a change to the hooks file.

To try a change to the installer without releasing, open a pull request (the dry run starts by itself) or run the workflow by hand with `action: dry-run`.

## The winget token

`winget-submit` needs a GitHub token of the repo owner's, because the pull request on `microsoft/winget-pkgs` comes from the owner's fork.

1. On GitHub: Settings → Developer settings → Personal access tokens → Tokens (classic) → Generate new token. Tick `public_repo` and nothing else. `wingetcreate` doesn't accept fine-grained tokens.
2. In this repo: Settings → Secrets and variables → Actions → New repository secret, named `WINGET_TOKEN`.

The token reaches `wingetcreate` through an environment variable and is never on a command line. Only `winget-submit` reads it, and that job doesn't run for pull requests.

The first pull request on `microsoft/winget-pkgs` gets a comment from Microsoft's bot asking for the contributor agreement. The repo owner answers it once, from the account that owns the token.

After a submission, open the pull request and look at it: a green `winget-submit` only means it was opened.

- Its body is the repository's template with nothing ticked. Tick what holds: the contributor agreement once it is signed, and `winget validate` and `winget install --manifest`, both of which `winget-check` runs.
- Microsoft's bot can close it. alpha.1's first one was closed within a minute, without a comment, together with every other pull request opened in those minutes. When that happens, wait until new pull requests there stay open and run `submit` again. Never run it while an earlier pull request for the same version is still open: that opens a duplicate.

## Holding the submission

A tag submits to winget by itself once `winget-check` passes. To hold that, set the repository variable `WINGET_HOLD` before you push the tag:

```sh
gh variable set WINGET_HOLD --body true
```

While it is set, a tag's run builds, publishes and runs `winget-check` as always, and `winget-submit` ends green with a notice that says how to submit later. Any value but `false` holds, so a typo holds too. To let tags submit again:

```sh
gh variable delete WINGET_HOLD
```

- A manual `submit` ignores the hold: it is a request for that one release.
- Change the variable before you push the tag, not while the run is going: when a running job reads it isn't pinned.
- Cancelling a tagged run stops the winget jobs as well, but a cancel holds nothing once `wingetcreate` has opened the pull request, and `winget-check` can pass in about a minute. Use the variable. Until slice 9c both jobs ignored a cancel altogether (issue #216).
- Re-running a held tag's run doesn't submit either: `publish` fails, because the release exists. Run `submit` by hand.

The hold is on as of 2026-11-26, until alpha.1's new-package pull request (microsoft/winget-pkgs#482096) is merged, because a submission before that would open a second "New package" pull request. Once #482096 is merged, delete the variable, sync the fork, and submit alpha.2 with `gh workflow run release.yml -f action=submit -f tag=v0.1.0-alpha.2`.

## The fork

`wingetcreate` pushes its branch to the token owner's fork of `microsoft/winget-pkgs` and opens the pull request from there. It stops when it can't bring the fork up to date first ("The forked repository could not be synced with the upstream commits", alpha.2). So `winget-submit` asks GitHub to sync the fork before `wingetcreate` runs, in its step "Bring the fork up to date", and stops with what to do when that fails:

- GitHub refuses the sync when `microsoft/winget-pkgs` has changed one of its own workflow files since the fork's last sync. The likely reason, not proven, is that the token has no `workflow` scope (`docs/research/2026-11-26-winget-hold-and-fork-sync.md`). Press **Sync fork** on the fork's page, then run `submit` again. Don't give the token the `workflow` scope: it would let the token change the workflows of every repository the owner has, this one's included.
- A fork with commits of its own on `master` isn't synced, because a merge would carry them toward the pull request. Put its `master` back to `microsoft/winget-pkgs`'s, then run `submit` again.
- A fork that doesn't exist yet is made by `wingetcreate`.

## A release that went out before the token, or whose submission failed

Sync the fork first (see [The fork](#the-fork)), then run the workflow by hand with `action: submit` and the release's tag:

```sh
gh workflow run release.yml -f action=submit -f tag=v0.1.0-alpha.1
```

A submission that stopped at "Bring the fork up to date" needs only that: nothing reached `microsoft/winget-pkgs`.

## When Microsoft hasn't accepted a version yet

Review takes from hours to days, and nothing guarantees that an unsigned alpha is accepted. Until then, and if it is refused, the release page's fallback works: the manifest folder in `winget-manifest-<version>.zip`, installed with `winget install --manifest`. The release notes explain it.

Once a version is in the community repository, check it end to end:

```sh
gh workflow run release.yml -f action=check -f tag=v0.1.0-alpha.1 -f source=community
```

## Known limits of the alpha installer

The release notes (`scripts/release/notes.mjs`) tell friends about each of these.

- Uninstalling from Windows' Settings runs with Explorer's environment. A `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME` or `QWEN_HOME` that is set only in a shell isn't seen, and what Marshell put in that folder stays: Claude Code's skill (its hooks file doesn't follow `CLAUDE_CONFIG_DIR`), Codex's hooks, Copilot CLI's hooks and status line, Qwen Code's hooks, status line and skill. `~/.marshell/bin` is removed all the same, so those hooks then point at a bridge that is gone. `winget uninstall` from that shell sees the variable: winget passes its environment to the uninstaller (`docs/research/2026-11-14-alpha-installer.md` section 3).
- A CLI file that can't be read or doesn't parse is left as it is, and `~/.marshell/bin` stays so the hooks in it keep working.
- Uninstalling with the setup program itself (running the installer over an install and choosing to uninstall, or cancelling right after its "uninstall before installing" step) removes the program and leaves the hooks. The uninstaller can't tell that from an update by hand, which must keep them. `%USERPROFILE%\.marshell\bin\marshell.exe uninstall` takes them out.
- The installer and the uninstaller close a running Marshell, and the sessions in it, before they replace or remove files. They don't close an administrator session's helper (`marshell-host.exe`) or a hook that is running at that moment. The programs Marshell started end a moment after the app, and a running program's file can't be deleted or replaced. The uninstaller therefore tries its program files again for up to 20 seconds; a program that is still running then keeps its file, and the install folder stays with it. The installer doesn't wait: a silent install over a running Marshell skips a file that is still in use and exits 0, so an update can keep an old `OpenConsole.exe` or `marshell-host.exe` (issue #214). Closing Marshell before `winget upgrade` avoids it; the release notes don't say that yet.
- Windows SmartScreen warns when the installer is run by hand, because it isn't signed. It warns the same way when winget starts the installer for an install from the manifest folder: winget marks the file it downloaded as coming from the internet and starts it through the shell, and it then waits, without a limit, at "Starting package install..." until the dialog is answered. On a runner nobody answers, so the smoke test sets `SEE_MASK_NOZONECHECKS=1` for winget, which makes the shell skip that check. With SmartScreen turned off by policy the shell shows "Open File - Security Warning" instead, and winget waits on that. Whether an install from the community repository meets the dialog isn't known until Microsoft accepts a version.
- An uninstall returns before it has finished, as every NSIS uninstaller does: it hands over to a copy of itself, which then removes the files and takes the hooks out (up to a couple of minutes when a WSL distro has to start). A `winget install` that meets it can lose the files it just wrote. Wait until `winget list Marshell` no longer shows it; when a program of Marshell's was still ending, the files can go up to 20 seconds after that.
- An install from the release page's manifest folder isn't tied to the package id until Microsoft accepts the package, so `winget upgrade Doofess.Marshell` and `winget uninstall Doofess.Marshell` don't find it. The notes give the forms that work.
- The installer carries `LICENSE` and `THIRD_PARTY_NOTICES.md` as they are in the repo. The full licence list for every bundled crate and package is phase 9 work that this slice doesn't do.

## Moving to a newer wingetcreate

`release.yml` pins `wingetcreate.exe` by its release address and SHA-256 (`WINGETCREATE_URL`, `WINGETCREATE_SHA256`). To move, take both from the new release:

```sh
gh api repos/microsoft/winget-create/releases/latest --jq '.tag_name, (.assets[] | select(.name == "wingetcreate.exe") | .digest)'
```

## Moving to a newer pinned action

The `build` job pins the two actions that aren't GitHub's own (`Swatinem/rust-cache`, `pnpm/action-setup`) to a commit, with the version in a comment. To move one, take the commit behind its release tag:

```sh
gh api repos/Swatinem/rust-cache/commits/v2.9.3 --jq .sha
```
