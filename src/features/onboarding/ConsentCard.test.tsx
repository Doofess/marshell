import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { ConsentCard } from "./ConsentCard";
import { CONSENT } from "./fixtures";
import { elements } from "../../test/hooks";
import { Switch } from "../../components/Switch/Switch";
import type { Capability } from "./consent";

const on = { status: true, approve: true, statusline: true };

describe("ConsentCard", () => {
  function switches(value: typeof on, offer?: Capability[]) {
    let tree: ReturnType<typeof ConsentCard>;
    const changes: Array<[Capability, boolean]> = [];
    function Capture() {
      tree = ConsentCard({ scenario: CONSENT.claude, on: value, offer, onSwitch: (c, v) => changes.push([c, v]) });
      return tree;
    }
    const html = renderToStaticMarkup(<Capture />);
    return { html, changes, toggle: (label: string, checked: boolean) => elements(tree).find((e) => e.type === Switch && e.props.label === label)!.props.onChange(checked) };
  }
  it("turns live status on when approval is enabled", () => {
    const card = switches({ status: false, approve: false, statusline: false });
    card.toggle("Approve from anywhere", true);
    expect(card.changes).toEqual([["status", true], ["approve", true]]);
    expect(card.html).toContain("Approve from anywhere needs live status to know when a request is answered.");
  });
  it("turns approval off when live status is disabled", () => {
    const card = switches(on);
    card.toggle("Show status and context", false);
    expect(card.changes).toEqual([["approve", false], ["status", false]]);
  });
  it("leaves the approval-only upgrade request intact", () => {
    const card = switches({ status: false, approve: false, statusline: false }, ["approve"]);
    card.toggle("Approve from anywhere", true);
    expect(card.changes).toEqual([["approve", true]]);
  });
  it("says what will change in plain words, with the backup and the way out", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />);
    expect(h).toContain("Add 11 hook entries and chain your status line");
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
  it("backs up an existing hooks.json too, and names files the same way with either slash", () => {
    const d = CONSENT.antigravity.detection;
    const h = renderToStaticMarkup(<ConsentCard scenario={{ ...CONSENT.antigravity, detection: { ...d, hooks: { path: "/home/u/.gemini/config/hooks.json", exists: true } } }} on={on} />);
    expect(h).toContain("Backups of both files are saved first, in <code>C:\\Users\\Home Office\\.marshell\\snapshots\\2026-10-10T20-15</code>.");
    expect(h).not.toContain("is a new file");
    const fresh = renderToStaticMarkup(<ConsentCard scenario={{ ...CONSENT.antigravity, detection: { ...d, hooks: { path: "/home/u/.gemini/config/hooks.json", exists: false } } }} on={on} />);
    expect(fresh).toContain("<code>hooks.json</code> is a new file, so it needs no backup.");
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
  it("leaves out a capability that is not offered", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} offer={["status", "statusline"]} />);
    expect(h).not.toContain("Approve from anywhere");
    expect(h.match(/role="switch"/g)).toHaveLength(2);
  });
  it("offers no status line switch when the user has none", () => {
    const d = { ...CONSENT.claude.detection, hasOwnStatusLine: false };
    const h = renderToStaticMarkup(<ConsentCard scenario={{ ...CONSENT.claude, detection: d }} on={on} offer={["status", "statusline"]} />);
    expect(h).not.toContain("Keep my status line working");
    expect(h).toContain("and a status line in");
  });
  it("on a conflict offers Review the difference and Reload, and no apply button", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} onApply={() => {}} conflict={{ onReload: () => {} }} />);
    expect(h).toContain("Review the difference");
    expect(h).toContain(">Reload<");
    expect(h).not.toContain("consent__apply");
  });
  it("has an apply button that busy disables, and an error that is an alert", () => {
    const idle = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} onApply={() => {}} />);
    expect(idle).toContain("consent__apply");
    expect(idle).not.toContain('role="alert"');
    const busy = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} onApply={() => {}} busy error="Disk full" />);
    expect(/<button[^>]*consent__apply[^>]*disabled/.test(busy)).toBe(true);
    expect(busy).toMatch(/role="alert"[^>]*>Disk full/);
  });
});
