# Releasing an alpha

Alpha builds are for people who want to try Marshell before v1. They install with winget, cost nothing to publish and are not signed. The decisions are in `docs/PLAN.md` §9, "Slice 9a decisions"; the evidence is in `docs/research/2026-11-14-winget-alpha.md` and `docs/research/2026-11-14-alpha-installer.md`.

## Cut a release

1. `main` is green and holds what the release should carry.
2. Run a dry run on `main` and wait for it: `gh workflow run release.yml -f action=dry-run`. A pull request runs one only when it touches the installer or the release scripts, and a change elsewhere (the CLI's uninstall, an adapter's hooks) can break what the smoke test checks. A tag whose build fails publishes nothing, but its number is used up.
3. Tag it and push the tag. The tag is the version: `v0.1.0-alpha.N`, with N one more than the last release.

   ```sh
   git tag v0.1.0-alpha.2
   git push origin v0.1.0-alpha.2
   ```

4. Watch the `release` workflow. Nothing else is needed: the version is stamped into the build from the tag, and no file in the repo changes.

A tag of any other form stops the workflow before it builds.

## What the workflow does

`.github/workflows/release.yml` has four jobs.

| Job | When | What it does |
| --- | --- | --- |
| `build` | A tag, a pull request that touches the installer or the release scripts, or a manual `dry-run` | Stamps the version (`0.1.0-alpha.0` for a dry run), builds the per-user NSIS installer, writes the winget manifest, the checksum and the release notes, and runs the smoke test on the installer. |
| `publish` | A tag, after `build` | Creates the GitHub pre-release with the installer, its `.sha256` and `winget-manifest-<version>.zip`. |
| `winget-check` | After `publish`, or a manual `check` | On a fresh runner: installs the release with winget from its manifest folder, starts the app and uninstalls it with winget. |
| `winget-submit` | After `winget-check` passes, or a manual `submit` | Opens the pull request on `microsoft/winget-pkgs` with Microsoft's `wingetcreate`. Without the token it says so and ends green. |

The smoke test is `scripts/release/smoke.ps1`. It installs silently, checks the uninstall entry and the installed files, starts the app and asks it for its sessions, updates over the running app, uninstalls, and checks what is left: no hooks in the CLI files, no program files, and `~/.marshell` kept without its `bin` folder. It runs on GitHub's runners. Don't run it on a PC where Marshell is installed or running; it refuses to start there.

To try a change to the installer without releasing, open a pull request (the dry run starts by itself) or run the workflow by hand with `action: dry-run`.

## The winget token

`winget-submit` needs a GitHub token of the repo owner's, because the pull request on `microsoft/winget-pkgs` comes from the owner's fork.

1. On GitHub: Settings → Developer settings → Personal access tokens → Tokens (classic) → Generate new token. Tick `public_repo` and nothing else. `wingetcreate` doesn't accept fine-grained tokens.
2. In this repo: Settings → Secrets and variables → Actions → New repository secret, named `WINGET_TOKEN`.

The token reaches `wingetcreate` through an environment variable and is never on a command line. Only `winget-submit` reads it, and that job doesn't run for pull requests.

The first pull request on `microsoft/winget-pkgs` gets a comment from Microsoft's bot asking for the contributor agreement. The repo owner answers it once, from the account that owns the token.

## A release that went out before the token, or whose submission failed

Run the workflow by hand with `action: submit` and the release's tag:

```sh
gh workflow run release.yml -f action=submit -f tag=v0.1.0-alpha.1
```

## When Microsoft hasn't accepted a version yet

Review takes from hours to days, and nothing guarantees that an unsigned alpha is accepted. Until then, and if it is refused, the release page's fallback works: the manifest folder in `winget-manifest-<version>.zip`, installed with `winget install --manifest`. The release notes explain it.

Once a version is in the community repository, check it end to end:

```sh
gh workflow run release.yml -f action=check -f tag=v0.1.0-alpha.1 -f source=community
```

## Known limits of the alpha installer

- Uninstalling from Windows' Settings runs with Explorer's environment. A `CLAUDE_CONFIG_DIR`, `CODEX_HOME` or `COPILOT_HOME` that is set only in a shell isn't seen, and what Marshell put in that folder (its skill, and for Codex and Copilot its hooks) stays. `winget uninstall` from that shell sees it.
- The installer closes a running Marshell before it replaces or removes files. It doesn't close an administrator session's helper (`marshell-host.exe`); a file in use stays until the next install.
- Windows SmartScreen warns when the installer is run by hand, because it isn't signed.
- An uninstall returns before it has finished, as every NSIS uninstaller does: it hands over to a copy of itself. `winget uninstall` followed at once by `winget install` can meet the uninstall still at work; wait a few seconds between them.

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
