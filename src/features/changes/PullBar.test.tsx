import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { CiCheck } from "../../generated/CiCheck";
import { ChangesHead } from "./ChangesPanel";
import { ChecksList, checkDuration, checkMenuItems, checkSentence, durationText, rovingKey } from "./ChecksList";
import { PullBar } from "./PullBar";
import { type BarView, barOf, checksOf } from "./pullModel";
import type { GithubReply } from "../../generated/GithubReply";
import type { PullStatus } from "../../generated/PullStatus";

const check = (o: Partial<CiCheck>): CiCheck => ({ kind: "run", state: "passed", name: "build", workflow: "ci", url: "https://github.com/octo-org/widgets/runs/1", run_id: 1, started_ms: null, completed_ms: null, ...o }) as CiCheck;

const pull = (checks: CiCheck[], more = 0): PullStatus => ({ id: "PR_1", number: 7, title: "t", url: "https://github.com/octo-org/widgets/pull/7", state: "open", base: "main", head: "feat", readiness: "ready", review: "none", checks, more_checks: more, auto_merge: null, can_auto_merge: false }) as PullStatus;

const noop = () => {};
const list = (checks: CiCheck[], agent: string | null = "Claude", more = 0) =>
  renderToStaticMarkup(<ChecksList view={checksOf(pull(checks, more))} agent={agent} now={1_000_000} id="c" onCheck={noop} onEscape={noop} />);
const bar = (view: BarView) => renderToStaticMarkup(<PullBar view={view} agent="Claude" now={0} onMain={noop} onMenu={noop} onCheck={noop} onEscape={noop} />);

const plain = (line: string, o: Partial<BarView> = {}): BarView => ({ line, second: null, notes: [], main: null, menu: [], checks: null, link: null, hide: false, scope: "other", ...o });

describe("PullBar", () => {
  it("draws the bar between the branch line and the diff", () => {
    const html = renderToStaticMarkup(<ChangesHead strip={<p>main · 2 ahead</p>} bar={<PullBar view={plain("No pull request")} agent={null} now={0} onMain={noop} onMenu={noop} onCheck={noop} onEscape={noop} />} />);
    expect(html.indexOf("main · 2 ahead")).toBeGreaterThan(-1);
    expect(html.indexOf('aria-label="Pull request"')).toBeGreaterThan(html.indexOf("main · 2 ahead"));
  });
  it("keeps the bar over an empty diff", () => {
    const html = renderToStaticMarkup(<ChangesHead strip={null} bar={<PullBar view={plain("#7 Merged")} agent={null} now={0} onMain={noop} onMenu={noop} onCheck={noop} onEscape={noop} />} />);
    expect(html).toContain('aria-label="Pull request"');
    expect(html).toContain("#7 Merged");
  });
  it("shows the gh line with Get it and Hide", () => {
    const view = barOf({ kind: "no_gh" } as GithubReply, null)!;
    const html = bar(view);
    expect(html).toContain("Pull requests need the GitHub CLI.");
    expect(html).toContain(">Get it<");
    expect(html).toContain(">Hide<");
  });
  it("keeps a button that is off reachable, with its reason", () => {
    const html = bar(plain("#7 Open", { main: { id: "merge", label: "Merge", disabled: "#7 has conflicts with main. Resolve them in the terminal.", method: "merge", others: [] } }));
    expect(html).toContain('aria-disabled="true"');
    expect(html).not.toMatch(/<button[^>]* disabled/);
    expect(html).toContain("#7 has conflicts with main. Resolve them in the terminal.");
    expect(html).toMatch(/aria-describedby="([^"]+)"/);
  });
  it("starts with the checks folded behind a disclosure", () => {
    const html = bar(plain("#7 Open", { checks: checksOf(pull([check({ state: "failed" })])) }));
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain("Checks: 1 failed");
    expect(html).not.toContain("Ask Claude to fix");
  });
  it("offers the method menu only with other methods", () => {
    const main = { id: "merge" as const, label: "Merge", disabled: null, method: "merge" as const };
    expect(bar(plain("x", { main: { ...main, others: [{ method: "squash", label: "Squash and merge" }] } }))).toContain("Choose how to merge");
    expect(bar(plain("x", { main: { ...main, others: [] } }))).not.toContain("Choose how to merge");
  });
});

describe("PullBar: a button that is off", () => {
  const others = [{ method: "squash" as const, label: "Squash and merge" }];
  it("offers no method menu while Merge is off", () => {
    const html = bar(plain("#7 Open", { main: { id: "merge", label: "Merge", disabled: "#7 has conflicts with main. Resolve them in the terminal.", method: "merge", others } }));
    expect(html).not.toContain("Choose how to merge");
  });
  it("draws a sentence that is both a note and the reason once, and the button names it", () => {
    const why = "origin pushes somewhere other than octo-org/widgets, so push from the terminal.";
    const html = bar(plain("No pull request · 1 commit to push", { notes: [why, "Another note."], main: { id: "create", label: "Create pull request…", disabled: why } }));
    expect(html.split(why).length - 1).toBe(1);
    expect(html).toContain("Another note.");
    const described = /aria-describedby="([^"]+)"/.exec(html)?.[1];
    expect(html).toContain(`<p id="${described}" class="pull-bar__note">${why}</p>`);
  });
});

describe("ChecksList", () => {
  const mixed = [check({ name: "ok1" }), check({ state: "failed", name: "clippy" }), check({ name: "ok2" }), check({ state: "skipped", name: "lint" })];
  it("lists failed checks first and folds passed ones", () => {
    const html = list(mixed);
    expect(html).toContain("clippy");
    expect(html).toContain("2 passed, 1 skipped");
    expect(html).not.toContain("ok1");
    expect(html.indexOf("clippy")).toBeLessThan(html.indexOf("2 passed, 1 skipped"));
  });
  it("is one tab stop", () => {
    expect(list(mixed).match(/tabindex="0"/g)).toHaveLength(1);
  });
  it("gives a failed check 'Ask Claude to fix' and a passed one none", () => {
    expect(list([check({ state: "failed" })])).toContain("Ask Claude to fix");
    expect(list([check({ state: "failed" })], null)).toContain("Copy a request to fix");
    expect(list([check({ state: "running" })])).not.toContain("to fix");
    expect(checkMenuItems(check({ state: "passed" }), "Claude").map((i) => ("id" in i ? i.id : ""))).toEqual(["open", "copy_link"]);
    expect(checkMenuItems(check({ state: "cancelled" }), "Claude").map((i) => ("id" in i ? i.id : ""))).toEqual(["open", "fix", "copy_link"]);
  });
  it("names each check's state in words", () => {
    const html = list([check({ state: "failed", name: "a" }), check({ state: "running", name: "b" }), check({ state: "waiting", name: "c" }), check({ state: "cancelled", name: "d" })]);
    for (const word of ["Failed: a", "Running: b", "Waiting: c", "Cancelled: d"]) expect(html).toContain(`aria-label="${word}`);
    expect(html).toContain(">Failed<");
  });
  it("ends with the checks GitHub has that the reply doesn't list", () => {
    expect(list([check({ state: "failed" })], "Claude", 4)).toContain("And 4 more on GitHub.");
    expect(list([check({ state: "failed" })])).not.toContain("more on GitHub");
  });
  it("reads a row as a sentence", () => {
    expect(checkSentence(check({ state: "failed", name: "clippy", started_ms: 1000, completed_ms: 49_000 }), 0)).toBe("Failed: clippy, ci, 48 s");
    expect(checkSentence(check({ state: "running", name: "test", started_ms: 880_000 }), 1_000_000)).toBe("Running: test, ci, 2 min");
    expect(checkSentence(check({ state: "passed", name: "build", started_ms: 0, completed_ms: 72_000 }), 0)).toBe("Passed: build, ci, 1 min 12 s");
    expect(checkSentence(check({ state: "passed", workflow: null }), 0)).toBe("Passed: build");
  });
  it("writes a duration", () => {
    expect(durationText(48_000)).toBe("48 s");
    expect(durationText(59_600)).toBe("1 min");
    expect(durationText(120_000)).toBe("2 min");
    expect(durationText(72_000)).toBe("1 min 12 s");
    expect(durationText(3_900_000)).toBe("1 h 5 min");
    expect(durationText(-1)).toBeNull();
    expect(checkDuration(check({ state: "passed", started_ms: 5, completed_ms: null }), 9)).toBeNull();
  });
  it("moves the one tab stop with Up, Down, Home and End and stops at the ends", () => {
    expect(rovingKey(4, 1, "ArrowDown")).toEqual({ focus: 2 });
    expect(rovingKey(4, 3, "ArrowDown")).toEqual({ focus: 3 });
    expect(rovingKey(4, 0, "ArrowUp")).toEqual({ focus: 0 });
    expect(rovingKey(4, 2, "Home")).toEqual({ focus: 0 });
    expect(rovingKey(4, 1, "End")).toEqual({ focus: 3 });
    expect(rovingKey(4, 1, "a")).toBeNull();
  });
});
