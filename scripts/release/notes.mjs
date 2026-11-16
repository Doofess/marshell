// The text of an alpha release's page on GitHub (PLAN §9, slice 9a decisions 2, 3, 6, 9 and 15).
// Usage: node scripts/release/notes.mjs --version <v> --sha256 <hash>
import { fileURLToPath } from "node:url";
import { installerName } from "./version.mjs";

export function releaseNotes({ version, sha256 }) {
  const installer = installerName(version);
  return `Marshell ${version} is an alpha build for Windows 11 (x64). Things will change and some will break. Please report what you find in [Issues](https://github.com/Doofess/marshell/issues).

## Install

\`\`\`
winget install Doofess.Marshell
\`\`\`

If winget can't find this version yet, Microsoft is still reviewing it. Install it from this page instead:

1. Download \`winget-manifest-${version}.zip\` below and extract it.
2. Once, in a terminal opened as administrator: \`winget settings --enable LocalManifestFiles\`
3. \`winget install --manifest <the extracted folder>\`

You can also download \`${installer}\` and run it.

The installer needs no administrator rights and installs for your user only.

**Windows will warn you.** The alpha builds aren't signed. When you run the installer yourself, SmartScreen shows "Windows protected your PC": choose **More info**, then **Run anyway**.

## Update

\`\`\`
winget upgrade Doofess.Marshell
\`\`\`

If you installed from this page (the manifest folder or the installer), winget doesn't know the package yet: install the new version from its release page the same way.

The alpha doesn't update itself. An update keeps your settings, your history and the hooks Marshell set up in your agent CLIs. Updating and uninstalling close Marshell and the sessions running in it, so finish what your agents are doing first.

## Uninstall

\`\`\`
winget uninstall Doofess.Marshell
\`\`\`

If you installed from this page: \`winget uninstall --manifest <the extracted folder>\`, or Windows Settings > Apps > Installed apps.

Uninstalling takes Marshell's hooks, its status line and its skill out of every agent CLI's files, in Windows and in each WSL distro, and removes the program. If a distro is stopped, the uninstall starts it to look. It goes on working for a moment after the command returns, so wait until \`winget list Marshell\` no longer shows it before you install again.

What stays:

- Your Marshell settings, history and the backups of every file Marshell changed, in \`%USERPROFILE%\\.marshell\`. For a full clean-up delete that folder and \`%LOCALAPPDATA%\\dev.marshell.app\`.
- What Marshell put in a CLI config folder that only a shell knows about. If you set \`CLAUDE_CONFIG_DIR\`, \`CODEX_HOME\`, \`COPILOT_HOME\` or \`QWEN_HOME\` in a shell profile and not in Windows' environment variables, uninstall from that shell (\`winget uninstall\`), not from Settings.
- A CLI file that Marshell can't read or that doesn't parse is left as it is, with Marshell's hooks still in it. The hooks keep working, because \`%USERPROFILE%\\.marshell\\bin\` then stays too.
- The hooks, if you uninstall with the setup program itself (run the installer again and choose to uninstall). Run \`%USERPROFILE%\\.marshell\\bin\\marshell.exe uninstall\` first, or afterwards: it shows what it will change and asks.

## Licences

Marshell is MIT-licensed. The installer puts \`LICENSE\` and \`THIRD_PARTY_NOTICES.md\` next to the program. The notices file isn't complete yet: it covers ConPTY, the fonts and a few of the Rust crates, and not the other crates or the JavaScript packages in the app. The complete list comes before v1. Until then, \`Cargo.lock\` and \`pnpm-lock.yaml\` in the repository list the crates and packages the build draws on, along with ones used only to build and test it.

## Checksum

SHA-256 of \`${installer}\`:

\`\`\`
${sha256}
\`\`\`
`;
}

function main() {
  const args = process.argv.slice(2);
  const opt = {};
  for (let i = 0; i < args.length; i += 2) {
    if (!args[i].startsWith("--") || args[i + 1] === undefined) {
      throw new Error(`Bad argument "${args[i]}".`);
    }
    opt[args[i].slice(2)] = args[i + 1];
  }
  if (!opt.version || !opt.sha256) {
    throw new Error("Usage: notes.mjs --version <v> --sha256 <hash>");
  }
  process.stdout.write(releaseNotes({ version: opt.version, sha256: opt.sha256 }));
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
