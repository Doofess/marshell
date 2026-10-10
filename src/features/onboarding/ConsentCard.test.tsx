import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { ConsentCard } from "./ConsentCard";
import { CONSENT } from "./fixtures";

const on = { status: true, approve: true, statusline: true };

describe("ConsentCard", () => {
  it("says what will change in plain words, with the backup and the way out", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />);
    expect(h).toContain("Add 9 hook entries and chain your status line");
    expect(h).toContain("A backup is saved first");
    expect(h).toContain(".marshell\\snapshots");
    expect(h).toContain("removes only the entries Marshell added");
    expect(h).toContain("Skip: use limited status");
  });
  it("has a switch for each capability and explains an unavailable one", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.antigravity} on={{ status: false, approve: false, statusline: true }} />);
    expect(h.match(/role="switch"/g)).toHaveLength(3);
    expect(h).toContain("Antigravity has no verified approval hook, so Marshell focuses the tab instead (verified when the adapter is built).");
    expect(h).toContain("Its hook events are verified when the adapter is built.");
  });
  it("hides the diff until asked, then shows the exact change", () => {
    expect(renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />)).not.toContain("diff__lines");
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} open />);
    expect(h).toContain("diff__lines");
    expect(h).toContain("marshell-hook.exe");
  });
  it("adds the trust step for Codex", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.codex} on={on} />);
    expect(h).toContain("trust them there");
  });
  it("replaces the controls with the reason when the file cannot be edited", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.unparsableAntigravity} on={on} />);
    expect(h).toContain("does not parse");
    expect(h).not.toContain('role="switch"');
    expect(h).toContain("Skip: use limited status");
  });
  it("lists a managed file read-only and offers no control for it", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claudeManaged} on={on} />);
    expect(h).toContain("managed-settings.json");
    expect(h).toContain("Marshell can&#x27;t change it");
  });
  it("names its buttons from the text they show", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
});
