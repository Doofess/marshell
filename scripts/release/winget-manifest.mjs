// Writes the three winget manifest files for a release.
// Usage: node scripts/release/winget-manifest.mjs --version <v> --installer <setup file> --out <folder> [--date YYYY-MM-DD]
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { installerUrl } from "./version.mjs";

const ID = "Doofess.Marshell";
const MANIFEST_VERSION = "1.10.0";

const schema = (type) =>
  `# yaml-language-server: $schema=https://aka.ms/winget-manifest.${type}.${MANIFEST_VERSION}.schema.json`;

export function manifests({ version, sha256, releaseDate }) {
  if (!/^[0-9A-F]{64}$/.test(sha256)) {
    throw new Error("sha256 must be 64 upper-case hex characters.");
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(releaseDate)) {
    throw new Error("releaseDate must read YYYY-MM-DD.");
  }
  const head = (type) => [
    schema(type),
    "",
    `PackageIdentifier: ${ID}`,
    `PackageVersion: ${version}`,
  ];
  const tail = (type) => [`ManifestType: ${type}`, `ManifestVersion: ${MANIFEST_VERSION}`, ""];
  const repo = "https://github.com/Doofess/marshell";
  return {
    [`${ID}.yaml`]: [...head("version"), "DefaultLocale: en-US", ...tail("version")].join("\n"),
    [`${ID}.installer.yaml`]: [
      ...head("installer"),
      "InstallerType: nullsoft",
      "Scope: user",
      "InstallModes:",
      "- interactive",
      "- silent",
      "UpgradeBehavior: install",
      `ReleaseDate: ${releaseDate}`,
      "Installers:",
      "- Architecture: x64",
      `  InstallerUrl: ${installerUrl(version)}`,
      `  InstallerSha256: ${sha256}`,
      "  ProductCode: Marshell",
      ...tail("installer"),
    ].join("\n"),
    [`${ID}.locale.en-US.yaml`]: [
      ...head("defaultLocale"),
      "PackageLocale: en-US",
      "Publisher: Doofess",
      "PublisherUrl: https://github.com/Doofess",
      `PublisherSupportUrl: ${repo}/issues`,
      "PackageName: Marshell",
      `PackageUrl: ${repo}`,
      "License: MIT",
      `LicenseUrl: ${repo}/blob/main/LICENSE`,
      "ShortDescription: A desktop terminal for running AI coding agents side by side.",
      "Tags:",
      "- terminal",
      "- ai",
      "- claude-code",
      "- codex",
      `ReleaseNotesUrl: ${repo}/releases/tag/v${version}`,
      ...tail("defaultLocale"),
    ].join("\n"),
  };
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
  if (!opt.version || !opt.installer || !opt.out) {
    throw new Error(
      "Usage: winget-manifest.mjs --version <v> --installer <setup file> --out <folder> [--date YYYY-MM-DD]",
    );
  }
  const sha256 = createHash("sha256").update(readFileSync(opt.installer)).digest("hex").toUpperCase();
  const releaseDate = opt.date ?? new Date().toISOString().slice(0, 10);
  const files = manifests({ version: opt.version, sha256, releaseDate });
  mkdirSync(opt.out, { recursive: true });
  for (const [name, text] of Object.entries(files)) writeFileSync(join(opt.out, name), text);
  console.log(sha256);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
