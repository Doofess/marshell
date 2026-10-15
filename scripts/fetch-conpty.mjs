// Fetches Microsoft's ConPTY (conpty.dll + OpenConsole.exe) from NuGet into src-tauri/resources/conpty,
// pinned by version and SHA-512. It is faster than the ConPTY built into Windows (see docs/spikes/S2-conpty.md).
// Run once on Windows: `pnpm fetch:conpty`. It does nothing when the pinned version is already there.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "1.25.260930003";
// catalogEntry.packageHash from https://api.nuget.org/v3/registration5-semver1/microsoft.windows.console.conpty/1.25.260930003.json
const SHA512 = "LuqMk9r5oTzeRne35IOlUdSQ7DkjorOIbcnODGZ81MvmUpTr/1VBQz3fAdCNCwkfuR0Mluq+H7rS3Hes/A1fOw==";
const URL = `https://www.nuget.org/api/v2/package/Microsoft.Windows.Console.ConPTY/${VERSION}`;

// Paths inside the package, per architecture.
const FILES = {
  x64: { "conpty.dll": "runtimes/win-x64/native/conpty.dll", "OpenConsole.exe": "build/native/runtimes/x64/OpenConsole.exe" },
  arm64: {
    "conpty.dll": "runtimes/win-arm64/native/conpty.dll",
    "OpenConsole.exe": "build/native/runtimes/arm64/OpenConsole.exe",
  },
};

// The package declares MIT but carries no licence file; this is the text from github.com/microsoft/terminal.
const LICENSE = `MIT License

Copyright (c) Microsoft Corporation. All rights reserved.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
`;

const dest = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "resources", "conpty");
const versionFile = join(dest, "VERSION");
const targets = Object.entries(FILES).flatMap(([arch, files]) =>
  Object.entries(files).map(([name, inPackage]) => ({ inPackage, to: join(dest, arch, name) })),
);

const upToDate =
  existsSync(versionFile) &&
  readFileSync(versionFile, "utf8").trim() === VERSION &&
  targets.every((t) => existsSync(t.to));
if (upToDate) {
  console.log(`ConPTY ${VERSION} is up to date in ${dest}`);
  process.exit(0);
}

console.log(`Downloading ConPTY ${VERSION} from NuGet...`);
const response = await fetch(URL);
if (!response.ok) {
  console.error(`download failed: HTTP ${response.status} for ${URL}`);
  process.exit(1);
}
const pkg = Buffer.from(await response.arrayBuffer());
const actual = createHash("sha512").update(pkg).digest("base64");
if (actual !== SHA512) {
  console.error(`hash mismatch: expected ${SHA512}, got ${actual}`);
  process.exit(1);
}

const work = mkdtempSync(join(tmpdir(), "marshell-conpty-"));
try {
  const pkgPath = join(work, "conpty.nupkg");
  writeFileSync(pkgPath, pkg);
  // Windows' own tar (bsdtar) reads zip files; a GNU tar earlier on PATH (Git's) would not.
  const tar = process.platform === "win32" ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "tar";
  execFileSync(tar, ["-xf", pkgPath, "-C", work, ...targets.map((t) => t.inPackage)]);
  for (const t of targets) {
    mkdirSync(dirname(t.to), { recursive: true });
    copyFileSync(join(work, t.inPackage), t.to);
    console.log(`  ${t.to}`);
  }
  writeFileSync(join(dest, "LICENSE"), LICENSE);
  // Last, so an interrupted run is retried.
  writeFileSync(versionFile, `${VERSION}\n`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
console.log(`ConPTY ${VERSION} fetched into ${dest}`);
