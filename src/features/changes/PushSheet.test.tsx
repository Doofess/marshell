import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { plan } from "./pullSheetFixtures";
import { PushSheet } from "./PushSheet";
import { type RunState, INITIAL_RUN } from "./pushRun";

const none = () => {};
const state = (o: Partial<RunState>): RunState => ({ ...INITIAL_RUN, ...o });
const view = (initial: RunState) => renderToStaticMarkup(<PushSheet tab="t1" announce={none} onClose={none} onDone={none} initial={initial} />);
const buttons = (h: string) => [...h.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("PushSheet", () => {
  it("names the folder, the branch and the commits", () => {
    const h = view(state({ kind: "confirm", plan: plan({ moved: true, uncommitted: 2, more: 3 }) }));
    expect(h).toContain("Push to GitHub");
    expect(h).toContain("C:/work/widgets");
    expect(h).toContain("feat → origin");
    expect(h).toContain("2 commits will be pushed to origin.");
    expect(h).toContain("aaaaaaa");
    expect(h).toContain("Add a thing");
    expect(h).toContain("And 3 more.");
    expect(h).toContain("2 files aren&#x27;t committed and stay out.");
    expect(h).toContain("This isn&#x27;t the folder the terminal started in.");
    expect(buttons(h)).toEqual(["Cancel", "Push 2 commits"]);
    expect(view(state({ kind: "confirm", plan: plan({ to_push: 1 }) }))).toContain("1 commit will be pushed to origin.");
  });
  it("shows git's output and Cancel while running", () => {
    const h = view(state({ kind: "running", id: "p1", plan: plan(), lines: ["Enumerating objects", "<b>x</b>"] }));
    expect(h).toContain("Pushing feat to origin…");
    expect(h).toContain("Enumerating objects");
    expect(h).toContain("&lt;b&gt;x&lt;/b&gt;");
    expect(buttons(h)).toEqual(["Cancel"]);
    expect(h).not.toContain("aria-live");
    expect(h).not.toContain('role="status"');
  });
  it("lets the keyboard scroll the output while a push runs", () => {
    const h = view(state({ kind: "running", id: "p1", plan: plan(), lines: ["a"] }));
    expect(h).toMatch(/<div[^>]*tabindex="0"[^>]*aria-label="git&#x27;s output"|<div[^>]*aria-label="git&#x27;s output"[^>]*tabindex="0"/);
  });
  it("keeps the output with Try again after a failure", () => {
    const h = view(state({ kind: "failed", plan: plan(), lines: ["remote: denied"], reason: "The remote refused the push." }));
    expect(h).toContain("remote: denied");
    expect(h).toMatch(/<p[^>]*tabindex="-1"[^>]*>The remote refused the push\./);
    expect(buttons(h)).toEqual(["Close", "Try again"]);
  });
  it("says a stopped push was stopped, and offers Close", () => {
    const h = view(state({ kind: "cancelled", plan: plan(), lines: [] }));
    expect(h).toContain("The push was stopped.");
    expect(buttons(h)).toEqual(["Close"]);
  });
  it("shows a refusal with Close only, and a plan being read", () => {
    expect(buttons(view(state({ kind: "refused", reason: "Nothing to push." })))).toEqual(["Close"]);
    expect(view(INITIAL_RUN)).toContain("Reading the checkout…");
  });
  it("moves focus to the reason after a failure, and to the stopped sentence after a cancel", () => {
    // Focus goes to the first `data-focus` element: after an end that is the sentence, never the output around it.
    for (const s of [state({ kind: "failed", plan: plan(), lines: ["remote: denied"], reason: "The remote refused the push." }), state({ kind: "cancelled", plan: plan(), lines: ["a"] })]) {
      const h = view(s);
      expect(h.match(/data-focus/g)).toHaveLength(1);
      expect(h).toMatch(/<p[^>]*data-focus=""[^>]*>The (remote refused the push|push was stopped)./);
    }
  });
  it("lets the keyboard scroll the output that is kept after a failure or a cancel", () => {
    for (const s of [state({ kind: "failed", plan: plan(), lines: ["remote: denied"], reason: "No." }), state({ kind: "cancelled", plan: plan(), lines: ["a"] })]) {
      expect(view(s)).toMatch(/<div[^>]*class="pull-sheet__body"[^>]*tabindex="0"[^>]*aria-label="git&#x27;s output"/);
    }
  });
  it("focuses the output while the push runs", () => {
    expect(view(state({ kind: "running", id: "p1", plan: plan(), lines: ["a"] }))).toMatch(/<div[^>]*class="pull-sheet__body"[^>]*data-focus=""/);
  });
  it("covers the page", () => {
    expect(readFileSync(new URL("./PullSheetParts.tsx", import.meta.url), "utf8")).toContain("useCoverPage(");
  });
});
