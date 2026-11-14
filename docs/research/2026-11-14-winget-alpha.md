# winget for the alpha builds

- Date: 2026-11-14
- Purpose: the user wants friends to install the alpha builds with `winget`, at no cost. This note records what the primary sources say, for PLAN §9 "Slice 9a decisions".
- How it was gathered: a research agent read the pages below and quoted the deciding sentences. The quotes were not re-read by hand. "Not verified" items are open; slice 9a settles them in practice.
- The case: the per-user NSIS installer Tauri 2 builds, unsigned (a certificate costs money, `docs/research/code-signing.md`), published on this repo's public GitHub Releases by a workflow that runs on a tag.

## 1. The community repository (`microsoft/winget-pkgs`)

- **Submitting is free.** It is a pull request on GitHub; no page mentions a fee.
- **No rule requires a signed installer.** What the policies ask is that the package is not malware: "If a package is flagged by any of the security scans in the validation pipeline, it cannot be accepted" ([Policies.md](https://raw.githubusercontent.com/microsoft/winget-pkgs/master/doc/Policies.md)). The submission page adds: "The package may be identified as malware by mistake" ([Submit your manifest](https://learn.microsoft.com/en-us/windows/package-manager/package/repository)).
- **The pipeline's labels that could hit an unsigned installer:** `Validation-Defender-Error` ("During dynamic testing, Microsoft Defender reported a problem") and `URL-Validation-Error` (a failed URL reputation test). `Validation-Executable-Error` means the pipeline was "unable to locate the primary application". On `Needs-Author-Feedback`: "If you do not address the issue within 10 days, the bot will close the pull request."
- **No policy mentions alpha, beta or pre-release software.** Policy 1.1.3 asks for "a valuable and quality user experience". Versions sort as expected: "`1.2.34` being considered a higher `Version` than `1.2.34-beta`" ([Authoring.md](https://github.com/microsoft/winget-pkgs/blob/master/doc/Authoring.md)).
- **The installer's address must be the publisher's own:** "The InstallerUrl must be the ISV's release location" (rule 1.1.4). A GitHub Release asset is that.
- **The id is `Publisher.Package`.** A third part is allowed ("Sometimes you might see additional values separated by a second period"), and some packages use it for a channel, but no channel convention is documented.
- **Review time.** Moderators "generally prioritize the oldest open pull requests that have completed validation" ([Moderation.md](https://github.com/microsoft/winget-pkgs/blob/master/doc/Moderation.md)). A first-hand report from July 2026 saw the same app take from about 2 hours to about 8 days ([MSEndpointMgr](https://msendpointmgr.com/2026/07/30/is-winget-enterprise-ready/)).
- Not verified: how the pipeline treats an unsigned NSIS installer from a publisher it has not seen, and whether moderators accept an alpha.

## 2. The manifest for Tauri's NSIS installer

- **`InstallerType: nullsoft`.** winget then passes the silent switch itself: "When Nullsoft or Inno are specified, the client will automatically set the silent and silent with progress install behaviors"; the switch for Nullsoft is `/S` ([manifest](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest)).
- **`Scope: user`.** Tauri's installer is per-user by default: "By default the installer will install your application for the current user only", and it "does not require Administrator privileges to run" ([Tauri: Windows installer](https://v2.tauri.app/distribute/windows-installer/)).
- **Tauri's own switches:** `/P` (passive), `/NS` (no shortcuts), `/UPDATE`, `/R` and `/ARGS`. `/S` is NSIS's built-in silent mode, which the template tests with `${Silent}` ([installer.nsi](https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)).
- **The manifest must match the Add/Remove Programs entry:** "The `PackageName` should match the entry made in Add / Remove Programs", and so should `Publisher`. The template writes `DisplayVersion` from the app's version, under a key named from the product name.
- **winget upgrades with the installer type it installed with:** "If it is already installed, the same installer type will be required to ensure a proper upgrade" ([settings](https://learn.microsoft.com/en-us/windows/package-manager/winget/settings)). So a copy installed from the NSIS installer keeps upgrading from NSIS installers.
- Not verified: the uninstaller's switches, and what winget does after the app's own updater has changed the installed version.

## 3. Submitting from the release workflow

- **wingetcreate (Microsoft's tool)** needs a classic token with the `public_repo` scope: "Fine-grained tokens are not supported" ([token.md](https://github.com/microsoft/winget-create/blob/main/doc/token.md)). It makes the fork of `winget-pkgs` itself. Its `update` command is "designed to update an existing manifest".
- **The first version is submitted by hand.** `wingetcreate new` asks its questions and ends with a pull request.
- **The community tools** ask the same: Komac "needs a classic GitHub token to submit PRs to `winget-pkgs`" ([Komac](https://github.com/russellbanks/Komac)); the winget-releaser action needs a classic token with `public_repo` and `workflow`, a fork under the same account, and "At least **one** version of your package should already be present" ([winget-releaser](https://github.com/vedantmgoyal9/winget-releaser)).
- None of them costs anything.

## 4. Routes that skip the community repository

| Route | What each friend does | Cost |
| --- | --- | --- |
| A manifest folder on the GitHub Release | Once, as administrator: `winget settings --enable LocalManifestFiles` ("this feature needs to be enabled by an administrator", [install](https://learn.microsoft.com/en-us/windows/package-manager/winget/install)). Then download the folder and run `winget install --manifest <folder>`. There is no `winget upgrade`; each update repeats it. | Free |
| A source of our own | `winget source add` as administrator ("using `add` requires administrator privileges", [source](https://learn.microsoft.com/en-us/windows/package-manager/winget/source)). | Microsoft's reference source runs on Azure Functions and Cosmos DB ([winget-cli-restsource](https://github.com/microsoft/winget-cli-restsource)); no free option is stated |

Windows SmartScreen warns on an unsigned installer's first run whichever route is used.
