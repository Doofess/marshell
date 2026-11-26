import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// How the release workflow is wired to the winget hold (PLAN §9, slice 9c). The file is read as text: a job
// runs from its "  <name>:" line to the next line indented by two spaces, and a step from its "      - " line
// to the next one. A comment between two steps falls into the step before it, which nothing here minds.
const lines = readFileSync(new URL("../../.github/workflows/release.yml", import.meta.url), "utf8").split(/\r?\n/);
const jobsAt = lines.indexOf("jobs:");

function jobNames(): string[] {
  return lines
    .slice(jobsAt + 1)
    .map((line) => /^  ([a-z][a-z-]*):$/.exec(line)?.[1])
    .filter((name): name is string => name !== undefined);
}

function job(name: string): string[] {
  const start = lines.indexOf(`  ${name}:`, jobsAt);
  if (jobsAt < 0 || start < 0) throw new Error(`release.yml has no job ${name}.`);
  let end = start + 1;
  while (end < lines.length && !/^  \S/.test(lines[end])) end++;
  return lines.slice(start, end);
}

type Step = { label: string; lines: string[] };

function steps(name: string): Step[] {
  const found: string[][] = [];
  for (const line of job(name)) {
    if (line.startsWith("      - ")) found.push([line]);
    else if (found.length > 0) found[found.length - 1].push(line);
  }
  return found.map((step) => {
    const id = step.map((line) => /^        id: (\S+)$/.exec(line)?.[1]).find(Boolean);
    const named = step.map((line) => /^(?:      - |        )name: (.+)$/.exec(line)?.[1]).find(Boolean);
    const uses = step.map((line) => /^      - uses: (\S+)$/.exec(line)?.[1]).find(Boolean);
    return { label: id ?? named ?? uses ?? step[0].trim(), lines: step };
  });
}

function step(name: string, label: string): Step {
  const found = steps(name).find((s) => s.label === label);
  if (!found) throw new Error(`${name} has no step ${label}.`);
  return found;
}

const trimmed = (s: Step) => s.lines.map((line) => line.trim());
const condition = (name: string) => job(name).filter((line) => /^    if: /.test(line));

describe("the release workflow and the winget hold", () => {
  it("a cancel stops the winget jobs", () => {
    expect(condition("winget-check")).toEqual([
      "    if: ${{ !cancelled() && (needs.publish.result == 'success' || (github.event_name == 'workflow_dispatch' && inputs.action == 'check')) }}",
    ]);
    expect(condition("winget-submit")).toEqual([
      "    if: ${{ !cancelled() && ((needs.publish.result == 'success' && needs.winget-check.result == 'success') || (github.event_name == 'workflow_dispatch' && inputs.action == 'submit')) }}",
    ]);
    expect(jobNames().length).toBeGreaterThan(3);
    for (const name of jobNames()) {
      for (const line of condition(name)) expect(line, name).not.toContain("always()");
    }
  });

  it("the build job tries the hooks' waits before the smoke test", () => {
    const check = step("build", "Check the installer hooks' waits");
    expect(trimmed(check)).toContain("timeout-minutes: 5");
    expect(trimmed(check)).toContain("shell: pwsh");
    expect(trimmed(check)).toContain("run: ./scripts/release/hooks-check.ps1");
    const labels = steps("build").map((s) => s.label);
    const at = labels.indexOf("Check the installer hooks' waits");
    expect(labels.indexOf("Build the installer")).toBeGreaterThan(-1);
    expect(at).toBeGreaterThan(labels.indexOf("Build the installer"));
    expect(at).toBeLessThan(labels.indexOf("Smoke test (install, start, update over it, uninstall)"));
  });

  it("the hold reaches the decision", () => {
    const release = trimmed(step("winget-submit", "release"));
    expect(release).toContain("WINGET_HOLD: ${{ vars.WINGET_HOLD }}");
    expect(release).toContain("EVENT: ${{ github.event_name }}");
    expect(release).toContain("HAS_TOKEN: ${{ secrets.WINGET_TOKEN != '' }}");
    expect(release).toContain("node scripts/release/winget-hold.mjs");
  });

  it("every later step reads the decision", () => {
    const all = steps("winget-submit");
    const after = all.slice(all.findIndex((s) => s.label === "release") + 1);
    expect(after.map((s) => s.label)).toEqual([
      "Download the release's manifest folder",
      "Download wingetcreate",
      "title",
      "Bring the fork up to date",
      "Submit to microsoft/winget-pkgs",
    ]);
    for (const s of after) expect(trimmed(s), s.label).toContain("if: steps.release.outputs.submit == 'true'");
    expect(lines.join("\n")).not.toContain("outputs.token");
  });

  it("the fork is synced before the submission", () => {
    const sync = step("winget-submit", "Bring the fork up to date");
    expect(trimmed(sync)).toContain("GH_TOKEN: ${{ secrets.WINGET_TOKEN }}");
    const text = sync.lines.join("\n");
    for (const part of ["merge-upstream", "ahead_by", "::stop-commands::"]) expect(text, part).toContain(part);
  });

  it("only winget-submit reads the token", () => {
    const reads = (block: string[]) => block.some((line) => line.includes("secrets.WINGET_TOKEN"));
    expect(jobNames().filter((name) => reads(job(name)))).toEqual(["winget-submit"]);
    expect(
      steps("winget-submit")
        .filter((s) => reads(s.lines))
        .map((s) => s.label),
    ).toEqual(["release", "Bring the fork up to date", "Submit to microsoft/winget-pkgs"]);
  });
});
