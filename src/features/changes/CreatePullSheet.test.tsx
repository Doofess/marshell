import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CreatePullSheet } from "./CreatePullSheet";
import { plan } from "./pullSheetFixtures";
import { type RunState, INITIAL_RUN } from "./pushRun";

const none = () => {};
const state = (o: Partial<RunState>): RunState => ({ ...INITIAL_RUN, ...o });
const view = (initial: RunState) => renderToStaticMarkup(<CreatePullSheet tab="t1" repo="octo-org/widgets" announce={none} onClose={none} onDone={none} initial={initial} />);
const buttons = (h: string) => [...h.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("CreatePullSheet", () => {
  it("names the folder, the branch, the base and the repository", () => {
    const h = view(state({ kind: "confirm", plan: plan({ title: "Add a thing" }) }));
    expect(h).toContain("Create pull request");
    expect(h).toContain("C:/work/widgets");
    expect(h).toContain("feat → main · octo-org/widgets");
    expect(h).toContain("2 commits will be pushed to origin.");
    expect(h).toContain("Title");
    expect(h).toContain("Description");
    expect(h).toContain("Base branch");
    expect(h).toContain("Create as draft");
    expect(h).toContain('value="Add a thing"');
    expect(buttons(h)).toEqual(["Cancel", "Push and create"]);
  });
  it("warns about uncommitted files and a moved folder", () => {
    const h = view(state({ kind: "confirm", plan: plan({ uncommitted: 1, moved: true }) }));
    expect(h).toContain("1 file isn&#x27;t committed and stays out.");
    expect(h).toContain("A push runs this repository&#x27;s hooks and settings.");
  });
  it("reads 'Create pull request' when nothing is to push", () => {
    const h = view(state({ kind: "confirm", plan: plan({ to_push: 0, commits: [] }) }));
    expect(buttons(h)).toEqual(["Cancel", "Create pull request"]);
    expect(h).not.toContain("will be pushed");
  });
  it("turns the button off with an empty title", () => {
    expect(view(state({ kind: "confirm", plan: plan({ title: "  " }) }))).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Push and create/);
  });
  it("gives the line that says a pull request is being created the focus", () => {
    const h = view(state({ kind: "creating", plan: plan({}) }));
    expect(h).toMatch(/<p class="close-dialog__line pull-sheet__notice" tabindex="-1" data-focus="">Creating the pull request/);
  });
  it("shows a refusal with Close only", () => {
    const h = view(state({ kind: "refused", reason: "Nothing differs from main." }));
    expect(h).toContain("Nothing differs from main.");
    expect(buttons(h)).toEqual(["Close"]);
    expect(h).not.toContain("Base branch");
  });
  it("keeps the form after a create failure, with GitHub's sentence", () => {
    const h = view(state({ kind: "create_failed", plan: plan(), reason: "A pull request already exists." }));
    expect(h).toMatch(/<p[^>]*tabindex="-1"[^>]*>A pull request already exists\./);
    expect(buttons(h)).toEqual(["Cancel", "Create pull request"]);
    expect(h).toContain("Base branch");
  });
});
