import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AgentInstall } from "../../generated/AgentInstall";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { SettingValue } from "../../generated/SettingValue";
import { CompanyView } from "./CompanySection";

const install = (over: Partial<AgentInstall> = {}): AgentInstall => ({ agent: "claude", found: true, version: "2.1.0", install: { kind: "native" }, update_command: "claude update", newer_version: null, ...over });
const view = (over: Partial<AgentSettingsView> = {}): AgentSettingsView => ({
  agent: "claude",
  path: "C:/u/.claude/settings.json",
  state: { kind: "ok" },
  curated: [
    { key: "model", value: "opus", scope: "user" },
    { key: "effortLevel", value: null, scope: "user" },
    { key: "theme", value: "dark", scope: "managed" },
  ],
  others: [],
  raw: '{\n  "model": "opus"\n}',
  editable: true,
  hash: null,
  text: null,
  ...over,
});
const noop = () => {};
const noEdit = { draft: {}, onDraft: noop, rawDraft: null, onRawDraft: noop, review: { kind: "editing" } as const, onReview: noop, onApply: noop, onBack: noop, onDiscard: noop, onReload: noop, onUndo: noop };
const render = (over: Partial<Parameters<typeof CompanyView>[0]> = {}) =>
  renderToStaticMarkup(<CompanyView agent="claude" name="Claude Code" install={install()} view={view()} running={0} mode="form" onMode={() => {}} onUpdate={() => {}} {...noEdit} {...over} />);

describe("CompanyView", () => {
  it("shows the version and an Update button when there is an update command", () => {
    const html = render();
    expect(html).toContain("Version 2.1.0");
    expect(html).toContain("Update Claude Code");
  });
  it("shows the manual line instead for an unknown install", () => {
    const html = render({ install: install({ update_command: null, install: { kind: "unknown" } }) });
    expect(html).not.toContain("Update Claude Code");
    expect(html).toContain("Marshell can&#x27;t tell how Claude Code was installed.");
    expect(html).toContain("<code>claude update</code>");
  });
  it("shows the newer version and the running note", () => {
    const html = render({ install: install({ newer_version: "2.2.0" }), running: 2 });
    expect(html).toContain("Version 2.2.0 is available.");
    expect(html).toContain("Claude Code is running in 2 tabs. The update may fail while they run.");
  });
  it("shows no scope badge for a setting the file doesn't set", () => {
    const html = render({ view: view({ curated: [{ key: "effortLevel", value: null, scope: "user" }] }) });
    expect(html).toContain("–");
    expect(html).not.toContain("data-scope");
  });
  it("shows an en dash for an unset curated key and the Managed badge", () => {
    const html = render();
    expect(html).toContain("Default model");
    expect(html).toContain("opus");
    expect(html).toContain("–");
    expect(html).toContain('data-scope="managed"');
    expect(html).toContain('data-scope="user"');
  });
  it("shows Other settings only when there are some", () => {
    expect(render()).not.toContain("Other settings in this file");
    const html = render({ view: view({ others: [{ key: "alwaysThinking", value: "true", scope: "user" }] }) });
    expect(html).toContain("<h3");
    expect(html).toContain("Other settings in this file");
    expect(html).toMatch(/<code[^>]*>alwaysThinking<\/code>/);
  });
  it("disables Raw file when raw is null", () => {
    expect(render({ view: view({ raw: null }) })).toMatch(/<input(?=[^>]*value="raw")(?=[^>]*disabled)/);
    expect(render()).not.toMatch(/<input(?=[^>]*value="raw")(?=[^>]*disabled)/);
  });
  it("shows the raw note and a read-only text area in raw mode", () => {
    const html = render({ mode: "raw" });
    expect(html).toContain("Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden.");
    expect(html).toMatch(/<textarea[^>]*readOnly=""/);
  });
  it("has no input but the two radios and no enabled textarea", () => {
    for (const mode of ["form", "raw"] as const) {
      const html = render({ mode, view: view({ others: [{ key: "x", value: "1", scope: "user" }] }) });
      const inputs = html.match(/<input[^>]*>/g) ?? [];
      expect(inputs).toHaveLength(2);
      for (const i of inputs) expect(i).toContain('type="radio"');
      for (const t of html.match(/<textarea[^>]*>/g) ?? []) expect(t).toContain('readOnly=""');
    }
  });
  it("shows the file line", () => {
    expect(render({ view: view({ state: { kind: "missing" } }) })).toContain("doesn&#x27;t exist yet");
  });
  it("still renders the heading while nothing has loaded", () => {
    expect(render({ install: null, view: null })).toContain("Claude Code");
  });

  describe("the editable form", () => {
    const mode: SettingValue = { key: "permissions.defaultMode", value: "plan", scope: "user", control: { kind: "choice", values: ["default", "acceptEdits", "plan"] } };
    const theme: SettingValue = { key: "theme", value: "custom:ocean", scope: "user", control: { kind: "choice", values: ["dark", "light"] } };
    const days: SettingValue = { key: "cleanupPeriodDays", value: "30", scope: "user", control: { kind: "number", min: 1 } };
    const effort: SettingValue = { key: "effortLevel", value: null, scope: "user", control: { kind: "text" } };
    const form = (curated: SettingValue[], over: Partial<AgentSettingsView> = {}) => view({ curated, hash: "h", text: "{}", ...over });
    it("lists Not set first and the choices after it", () => {
      const h = render({ view: form([mode]) });
      expect(h).toMatch(/<select[^>]*><option value="">Not set<\/option>/);
      expect(h).toContain(">Accept edits</option>");
      expect(h).toMatch(/<option value="plan" selected="">Plan<\/option>/);
    });
    it("offers a value outside the list as from the file, selected", () => {
      expect(render({ view: form([theme]) })).toMatch(/<option value="custom:ocean" selected="">custom:ocean \(from the file\)<\/option>/);
    });
    it("labels each control and links its help", () => {
      const h = render({ view: form([days, effort]) });
      expect(h).toMatch(/<label[^>]*for="[^"]+"[^>]*>Delete transcripts after \(days\)<\/label>/);
      expect(h).toContain('type="number"');
      expect(h).toContain('min="1"');
      expect(h).toContain('type="text"');
      expect(h.match(/Empty means not set\./g)).toHaveLength(2);
      expect(h).toMatch(/aria-describedby="[^"]+"/);
    });
    it("gives a switch Not set, On and Off", () => {
      const sw: SettingValue = { key: "check_for_update_on_startup", value: "true", scope: "user", control: { kind: "switch" } };
      const h = render({ agent: "codex", view: form([sw], { agent: "codex" }) });
      expect(h).toMatch(/Not set<\/option><option value="on" selected="">On<\/option><option value="off">Off<\/option>/);
    });
    it("shows a field error under the control", () => {
      const h = render({ view: form([days]), draft: { cleanupPeriodDays: "0" } });
      expect(h).toContain("Use a whole number of at least 1.");
      expect(h).toContain('aria-invalid="true"');
    });
    it("gives a managed row no control and the managed line", () => {
      const m: SettingValue = { key: "model", value: "opus", scope: "managed", control: { kind: "text" }, lock: { kind: "managed", path: "C:/ProgramData/managed.json" } };
      const h = render({ view: form([m]) });
      expect(h).not.toContain("<select");
      expect(h).not.toContain('type="text"');
      expect(h).toContain("Set by C:/ProgramData/managed.json, so Marshell can&#x27;t change it here.");
    });
    it("gives a table row its two copies", () => {
      const t: SettingValue = { key: "model", value: "x", scope: "user", control: { kind: "text" }, lock: { kind: "not_scalar" } };
      expect(render({ view: form([t]) })).toContain("Set as a table in the file. Edit it in Raw file.");
      const h = render({ view: form([t], { text: null }) });
      expect(h).toContain("Set as a table in the file.");
      expect(h).not.toContain("Edit it in Raw file.");
    });
    it("explains a hidden row", () => {
      const t: SettingValue = { key: "model", value: "hidden", scope: "user", control: { kind: "text" }, lock: { kind: "hidden" } };
      expect(render({ view: form([t]) })).toContain("Holds a value shown as hidden, so the form can&#x27;t change it.");
    });
    it("makes the raw text editable when the core sent it, and says why not when it did not", () => {
      const on = render({ mode: "raw", view: form([mode], { text: "a = 1\n" }), rawDraft: "a = 2\n" });
      expect(on).toMatch(/<textarea[^>]*aria-label="C:\/u\/.claude\/settings.json"/);
      expect(on).toContain("raw-editor__text");
      expect(on).not.toMatch(/<textarea[^>]*readOnly/);
      expect(on).toContain("a = 2");
      expect(on).toContain("This is the file itself. Review shows the exact change before anything is written.");
      const off = render({ mode: "raw", view: form([mode], { text: null }) });
      expect(off).toMatch(/<textarea[^>]*readOnly/);
      expect(off).toContain("Raw editing is off: this file holds secret values, and they show as hidden here.");
    });
    it("shows the change strip only while there is a change", () => {
      expect(render({ view: form([days]) })).not.toContain("Review changes");
      const h = render({ view: form([days]), draft: { cleanupPeriodDays: "45" } });
      expect(h).toContain("1 change");
      expect(h).toContain("Review changes");
      expect(h).toContain("Discard");
      const bad = render({ view: form([days]), draft: { cleanupPeriodDays: "0" } });
      expect(bad).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Review changes/);
      expect(render({ mode: "raw", view: form([days]), rawDraft: "x" })).toContain("Raw file changed");
    });
    it("counts a raw change only while the raw file is shown, so the strip matches the change sent", () => {
      const v = form([days], { text: "a = 1\n" });
      expect(render({ mode: "raw", view: v, rawDraft: "a = 2\n" })).toContain("Raw file changed");
      const h = render({ mode: "form", view: v, rawDraft: "a = 2\n" });
      expect(h).not.toContain("Raw file changed");
      expect(h).not.toContain("Review changes");
    });
    it("keeps the strip and the busy button while planning, and with an error", () => {
      const h = render({ view: form([days]), draft: { cleanupPeriodDays: "45" }, review: { kind: "planning" } });
      expect(h).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Review changes/);
      expect(h).not.toMatch(/<button[^>]*\sdisabled=""/);
      const e = render({ view: form([days]), draft: { cleanupPeriodDays: "45" }, review: { kind: "error", message: "The core said no." } });
      expect(e).toContain("The core said no.");
      expect(e).toContain("Review changes");
    });
    const plan = (normalized: boolean) => ({ preview: { changeset_id: "c", title: "t", reason: "r", stale: false, can_apply: true, files: [{ path: "p", diff: "--- a/p\n+++ b/p\n@@ -1 +1 @@\n-a\n+b\n" }] } as unknown as ConfigPreview, normalized });
    it("shows the review with the diff, the backup note and Apply", () => {
      const h = render({ view: form([days]), draft: { cleanupPeriodDays: "45" }, review: { kind: "review", plan: plan(false) } });
      expect(h).toContain('aria-label="Changes to C:/u/.claude/settings.json"');
      expect(h).toMatch(/A backup is saved first, in <code>~\/\.marshell\/snapshots\/<\/code>\. To undo, Marshell puts that backup back, for as long as the file is unchanged since this change\./);
      expect(h).toContain(">Apply</button>");
      expect(h).toContain("Back to editing");
      expect(h).not.toContain("The file keeps its comments");
      expect(h).not.toContain("Review changes");
    });
    it("puts the normalized note first", () => {
      const h = render({ view: form([days]), review: { kind: "review", plan: plan(true) } });
      const note = "Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden. The file keeps its comments and layout.";
      expect(h).toContain(note);
      expect(h.indexOf(note)).toBeLessThan(h.indexOf("A backup is saved first"));
    });
    it("disables Apply while applying", () => {
      expect(render({ view: form([days]), review: { kind: "applying" } })).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Apply/);
    });
    it("shows saved, conflict and error copy", () => {
      expect(render({ view: form([days]), review: { kind: "saved", snapshotId: "s1" } })).toMatch(/Saved\. New Claude Code sessions use it\.[\s\S]*>Undo<\/button>/);
      const c = render({ view: form([days]), review: { kind: "conflict" } });
      expect(c).toContain("C:/u/.claude/settings.json changed since you opened it. Reload to see it.");
      expect(c).toContain(">Reload</button>");
    });
    it("shows an error with no pending change in the result area, where focus can land", () => {
      const h = render({ view: form([days]), review: { kind: "error", message: "C:/u/.claude/settings.json changed since this change, so undo would overwrite it." } });
      expect(h).toMatch(/<div class="company__result" tabindex="-1" data-focus="result"><p class="change-bar__error">C:\/u\/\.claude\/settings\.json changed since this change, so undo would overwrite it\.<\/p><\/div>/);
      expect(h).not.toContain("Review changes");
      const s = render({ view: form([days]), draft: { cleanupPeriodDays: "45" }, review: { kind: "error", message: "The core said no." } });
      expect(s).toMatch(/<p class="change-bar__error" tabindex="-1" data-focus="result">The core said no\.<\/p>/);
      expect(s.match(/The core said no\./g)).toHaveLength(1);
    });
    it("keeps Saved and a busy Undo while undoing", () => {
      const h = render({ view: form([days]), review: { kind: "undoing", snapshotId: "s1" } });
      expect(h).toContain("Saved. New Claude Code sessions use it.");
      expect(h).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Undo/);
    });
    it("says when Reload dropped the raw edit", () => {
      const h = render({ mode: "raw", view: form([days], { text: "a = 1\n" }), rawDropped: true });
      expect(h).toContain("C:/u/.claude/settings.json changed, so your raw edit was dropped. The text is the file as it is now.");
      expect(render({ mode: "raw", view: form([days], { text: "a = 1\n" }) })).not.toContain("raw edit was dropped");
    });
    it("marks where focus goes after each step: the review and the result line take focus, Review changes is the way back", () => {
      expect(render({ view: form([days]), review: { kind: "review", plan: plan(false) } })).toMatch(/<section class="change-review" aria-label="Review of the changes" tabindex="-1" data-focus="review">/);
      expect(render({ view: form([days]), review: { kind: "saved", snapshotId: "s1" } })).toMatch(/<div class="company__result" tabindex="-1" data-focus="result">/);
      expect(render({ view: form([days]), review: { kind: "conflict" } })).toMatch(/<div class="company__result" tabindex="-1" data-focus="result">/);
      expect(render({ view: form([days]), draft: { cleanupPeriodDays: "45" } })).toMatch(/<button[^>]*data-focus="review-button"[^>]*>Review changes/);
    });
    it("shows no controls for a file that does not parse", () => {
      const h = render({ view: view({ state: { kind: "unparsable", line: 1, message: "x" } as never, editable: true, curated: [{ key: "model", value: null, scope: "user" }] }) });
      expect(h).not.toContain("<select");
      expect(h).not.toContain('type="text"');
      expect(h).not.toContain("Review changes");
    });
    it("renders Antigravity read-only: no select, no strip", () => {
      const h = render({ agent: "antigravity", view: view({ agent: "antigravity", editable: false, curated: [mode, days] }), draft: { cleanupPeriodDays: "45" } });
      expect(h).not.toContain("<select");
      expect(h).not.toContain('type="number"');
      expect(h).not.toContain("Review changes");
      expect(h).not.toContain("Empty means not set.");
    });
  });
});
