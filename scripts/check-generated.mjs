// Fails CI when generated TypeScript uses bigint or is out of date with the Rust types.
import { execSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";

const dir = "src/generated";
const withBigint = readdirSync(dir).filter((f) => readFileSync(`${dir}/${f}`, "utf8").includes("bigint"));
if (withBigint.length > 0) {
  console.error(`bigint in generated types; add #[ts(type = "number")] to: ${withBigint.join(", ")}`);
  process.exit(1);
}
const stale = execSync(`git status --porcelain -- ${dir}`, { encoding: "utf8" }).trim();
if (stale) {
  console.error(`src/generated is stale. Run \`cargo test -p marshell-protocol\` and commit:\n${stale}`);
  process.exit(1);
}
