// Stamps the version into Cargo.toml, src-tauri/tauri.conf.json and package.json.
// Usage: node scripts/release/set-version.mjs --tag v0.1.0-alpha.1 | --dry-run
// Prints the version as the only line on stdout.
import { readFileSync, writeFileSync } from "node:fs";
import { DRY_RUN_VERSION, stamp, versionFromTag } from "./version.mjs";

const root = new URL("../../", import.meta.url);
const paths = {
  cargoToml: new URL("Cargo.toml", root),
  tauriConf: new URL("src-tauri/tauri.conf.json", root),
  packageJson: new URL("package.json", root),
};

try {
  const args = process.argv.slice(2);
  let version;
  if (args.length === 1 && args[0] === "--dry-run") {
    version = DRY_RUN_VERSION;
  } else if (args.length === 2 && args[0] === "--tag") {
    version = versionFromTag(args[1]);
  } else {
    throw new Error("Usage: set-version.mjs --tag <tag> | --dry-run");
  }
  const input = Object.fromEntries(
    Object.entries(paths).map(([key, url]) => [key, readFileSync(url, "utf8")]),
  );
  const output = stamp(input, version);
  for (const [key, url] of Object.entries(paths)) writeFileSync(url, output[key]);
  console.log(version);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
