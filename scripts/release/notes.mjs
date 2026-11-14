// The text of an alpha release's page on GitHub (PLAN §9, slice 9a decisions 2, 3, 6 and 9).
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

The alpha doesn't update itself. An update keeps your settings, your history and the hooks Marshell set up in your agent CLIs.

## Uninstall

\`\`\`
winget uninstall Doofess.Marshell
\`\`\`

Uninstalling takes Marshell's hooks and its skill out of every agent CLI's files, in Windows and in each WSL distro, and removes the program. If a distro is stopped, the uninstall starts it to look.

Two things stay:

- Your Marshell settings, history and the backups of every file Marshell changed, in \`%USERPROFILE%\\.marshell\`. Delete that folder yourself for a full clean-up.
- Hooks in a CLI config folder that only a shell knows about. If you set \`CLAUDE_CONFIG_DIR\`, \`CODEX_HOME\` or \`COPILOT_HOME\` in a shell profile and not in Windows' environment variables, uninstall from that shell (\`winget uninstall\`), not from Settings.

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
