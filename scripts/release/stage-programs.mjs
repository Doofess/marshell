// Copies the three helper programs the app looks for next to its own exe from `target/release` into
// `src-tauri/staged/`, where `src-tauri/tauri.release.conf.json` bundles them from (PLAN §9, slice 9a decision 14).
//
// They are staged, not bundled straight from `target/release`: tauri-build also copies every resource next to the
// built exe, which for a file already in `target/release` would be a copy onto itself. The base config leaves them
// out, because `tauri dev` and CI build with an empty `target/release`.
//
// Run by the release config's `beforeBuildCommand`, after `pnpm build:hook:release`.

import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PROGRAMS = ["marshell-hook.exe", "marshell.exe", "marshell-host.exe"];

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const from = join(root, "target", "release");
const to = join(root, "src-tauri", "staged");

const missing = PROGRAMS.filter((name) => !existsSync(join(from, name)));
if (missing.length > 0) {
  console.error(`Not built in ${from}: ${missing.join(", ")}. Run pnpm build:hook:release first.`);
  process.exit(1);
}

mkdirSync(to, { recursive: true });
for (const name of PROGRAMS) {
  copyFileSync(join(from, name), join(to, name));
  console.log(`staged ${name}`);
}
