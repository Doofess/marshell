// Scrubs recorded CLI fixtures of the home path, user name, ids and e-mail before they are committed.
// Usage: node scripts/scrub-fixtures.mjs <dir>   (rewrites every .json file under <dir> in place)
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const UUID = /[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}/g;
const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+/g;

function replaceAll(text, from, to) {
  return from ? text.split(from).join(to) : text;
}

/**
 * @param {string} text
 * @param {{ home: string, user: string }} opts
 * @param {{ uuids: Map<string, string> }} [state] shared across the files of one run, so ids link up
 */
export function scrub(text, { home, user }, state = { uuids: new Map() }) {
  const parts = home.replace(/\//g, "\\").split("\\").filter(Boolean); // ["C:", "Users", "<user>"]
  const drive = parts[0].replace(":", "");
  const rest = parts.slice(1, -1);
  const spell = (name, sep, lead) => `${lead}${[...rest, name].join(sep)}`;
  const spellings = (name) => [
    [spell(name, "\\\\", `${drive}:\\\\`), "\\\\"],
    [spell(name, "\\", `${drive}:\\`), "\\"],
    [spell(name, "/", `${drive}:/`), "/"],
    [spell(name, "/", `/${drive.toLowerCase()}/`), "/"],
  ];
  const targets = spellings("user");
  let out = text;
  spellings(user).forEach(([from], i) => {
    out = replaceAll(out, from, targets[i][0]);
  });
  out = replaceAll(out, user.replace(/ /g, "-"), "user");
  out = replaceAll(out, user, "user");
  out = out.replace(UUID, (id) => {
    const key = id.toLowerCase();
    if (!state.uuids.has(key)) {
      const n = String(state.uuids.size + 1).padStart(12, "0");
      state.uuids.set(key, `00000000-0000-4000-8000-${n}`);
    }
    return state.uuids.get(key);
  });
  return out.replace(EMAIL, "user@example.com");
}

function jsonFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return jsonFiles(p);
    return e.name.endsWith(".json") ? [p] : [];
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const dir = process.argv[2];
  if (!dir) {
    console.error("usage: node scripts/scrub-fixtures.mjs <dir>");
    process.exit(1);
  }
  const home = homedir();
  const opts = { home, user: basename(home) };
  const state = { uuids: new Map() };
  const files = jsonFiles(dir).sort();
  for (const f of files) {
    writeFileSync(f, scrub(readFileSync(f, "utf8"), opts, state));
  }
  console.log(`scrubbed ${files.length} file(s), ${state.uuids.size} distinct id(s)`);
}
