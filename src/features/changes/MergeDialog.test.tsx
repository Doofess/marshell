import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MergeDialog } from "./MergeDialog";

const none = () => {};
const pull = { id: "PR_1", number: 182, title: "Fix <b>it</b>", head: "feat", base: "main" };
const view = (o: Partial<Parameters<typeof MergeDialog>[0]> = {}) =>
  renderToStaticMarkup(<MergeDialog tab="t1" checkout="C:/work/widgets" pull={pull} method="squash" auto={false} announce={none} onClose={none} onDone={none} {...o} />);
const buttons = (h: string) => [...h.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("MergeDialog", () => {
  it("names the pull request, the method and the base", () => {
    const h = view();
    expect(h).toMatch(/<h2[^>]*>Merge #182\?<\/h2>/);
    expect(h).toContain("Squash and merge &quot;Fix &lt;b&gt;it&lt;/b&gt;&quot; (#182) into main.");
    expect(buttons(h)).toEqual(["Cancel", "Squash and merge"]);
    expect(h).not.toContain("once its checks pass");
  });
  it("reads as a sentence for every method", () => {
    expect(view({ method: "merge" })).toContain("Merge &quot;Fix &lt;b&gt;it&lt;/b&gt;&quot; (#182) into main.");
    expect(view({ method: "rebase" })).toContain("Rebase and merge &quot;Fix &lt;b&gt;it&lt;/b&gt;&quot; (#182) into main.");
  });
  it("says GitHub merges once checks pass for auto-merge", () => {
    const h = view({ auto: true });
    expect(h).toContain("GitHub will merge it once its checks pass.");
    expect(buttons(h)).toEqual(["Cancel", "Merge when checks pass"]);
  });
  it("shows GitHub's sentence on a failure and keeps both buttons", () => {
    const h = view({ initialError: "Base branch was modified." });
    expect(h).toMatch(/<p[^>]*tabindex="-1"[^>]*>Base branch was modified\./);
    expect(buttons(h)).toEqual(["Cancel", "Squash and merge"]);
  });
  it("disables the buttons while it runs", () => {
    expect(view({ initialBusy: true }).match(/aria-disabled="true"/g)).toHaveLength(2);
  });
});
