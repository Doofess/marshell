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
    expect(h).toContain("removes the hook entries it added and puts your own status line back from statusline-chain.json");
    expect(h).toContain("Skip: use limited status");
  });
  it("has a switch for each capability and explains an unavailable one", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.codex} on={on} />);
    expect(h.match(/role="switch"/g)).toHaveLength(3);
    expect(h).toContain("Codex has no status line to chain.");
  });
  it("names both Antigravity files: hooks in its shared hooks.json, the status line in its own settings", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.antigravity} on={on} open />);
    expect(h).toContain("Add 5 hook entries in C:\\Users\\Home Office\\.gemini\\config\\hooks.json (a new file) and chain your status line in C:\\Users\\Home Office\\.gemini\\antigravity-cli\\settings.json.");
    expect(h).toContain("Exact changes to C:\\Users\\Home Office\\.gemini\\config\\hooks.json and C:\\Users\\Home Office\\.gemini\\antigravity-cli\\settings.json");
    expect(h).toContain("<code>hooks.json</code> is a new file, so it needs no backup.");
    expect(h).not.toContain("verified when the adapter is built");
    expect(h).not.toContain("/hooks");
  });
  it("offers to hand the mode to Marshell when the CLI's own settings skip every prompt, and says what that setting does", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.antigravityHandOver} on={on} />);
    expect(h).toContain("Let Marshell set the mode");
    expect(h).toContain("Antigravity CLI is set to skip every permission prompt (<code>dangerouslySkipPermissions</code>), so Marshell can&#x27;t ask you about anything.");
    expect(h).toContain("Turning it off lets each session&#x27;s mode decide; Bypass keeps today&#x27;s behaviour.");
    expect(h.match(/role="switch"/g)).toHaveLength(4);
    // The switch shows the plan's choice: on here, so the headline and the diff turn the setting off.
    const handOver = /<input([^>]*)>(?:(?!<input)[\s\S])*?Let Marshell set the mode/.exec(h)![1]!;
    expect(handOver).toContain('checked=""');
    // Without such a setting there is nothing to hand over.
    expect(renderToStaticMarkup(<ConsentCard scenario={CONSENT.antigravity} on={on} />)).not.toContain("Let Marshell set the mode");
    expect(renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />)).not.toContain("Let Marshell set the mode");
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
