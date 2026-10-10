import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { applyChanges, previewChange } from "./edit";
import { FieldRow } from "./FieldRow";
import { ScopeBadge } from "./ScopeBadge";
import { SettingsPage } from "./SettingsPage";
import { SETTINGS_TEXT, SETTINGS_TEXT_BROKEN } from "./fixtures";
import type { SettingField } from "./fields";
import { SCOPE_NOTE } from "./scope";
import { settingsPath } from "./settingsUi";

const base = { category: "claude", mode: "form" as const };
const page = (o: Partial<Parameters<typeof SettingsPage>[0]> = {}) => renderToStaticMarkup(<SettingsPage {...base} state={{}} {...o} />);

describe("a file that does not parse", () => {
  it("is never parsed for a preview, so nothing throws and there is nothing to review", () => {
    expect(() => previewChange("p.json", SETTINGS_TEXT_BROKEN, { model: "sonnet" })).not.toThrow();
    expect(previewChange("p.json", SETTINGS_TEXT_BROKEN, { model: "sonnet" })).toBe("");
    expect(applyChanges(SETTINGS_TEXT_BROKEN, { model: "sonnet" })).toBeUndefined();
    expect(applyChanges(SETTINGS_TEXT, { model: "sonnet" })).toContain('"model": "sonnet"');
  });
  it("shows no review and no Apply even when edits are pending, and does not crash", () => {
    const h = page({ state: { unparsable: true, changed: true, reviewing: true }, edits: { model: "sonnet" }, opened: SETTINGS_TEXT_BROKEN });
    expect(h).not.toContain("change-review");
    expect(h).not.toContain("change-bar");
    expect(h).not.toContain(">Apply<");
  });
  it("names the file, the line and the column in form mode too", () => {
    const h = page({ state: { unparsable: true }, opened: SETTINGS_TEXT_BROKEN });
    expect(h).toContain("does not parse (line 3, column ");
    expect(h).toContain("settings.json");
    expect(h).toContain("so Marshell won&#x27;t edit it.");
  });
  it("says the line is unknown with an en dash when it cannot tell", () => {
    expect(page({ state: { unparsable: true }, opened: SETTINGS_TEXT })).toContain("(line –)");
  });
  it("shows an en dash for each value it cannot read, never the values from the broken file", () => {
    const h = page({ state: { unparsable: true }, opened: SETTINGS_TEXT_BROKEN });
    expect((h.match(/data-unknown/g) ?? []).length).toBeGreaterThanOrEqual(2);
    expect(h).toContain('value="–"');
    expect(h).not.toContain('value="30"');
  });
});

describe("what the strip counts and previews", () => {
  it("counts only edits that differ from the current value", () => {
    const h = page({ state: { changed: true }, edits: { cleanupPeriodDays: 365, model: "opus" } });
    expect(h).toContain(">1 change<");
  });
  it("renders no strip at all when every edit equals the current value: there is nothing to review or apply", () => {
    const h = page({ state: { changed: true }, edits: { cleanupPeriodDays: 30 } });
    expect(h).not.toContain("change-bar");
    expect(h).not.toContain("No changes");
  });
  it("says 'Changes to the file' for raw text, which has no field edits to count", () => {
    expect(page({ mode: "raw", state: { changed: true } })).toContain(">Changes to the file<");
  });
  it("previews only the real changes", () => {
    const h = page({ state: { changed: true, reviewing: true }, edits: { cleanupPeriodDays: 365, model: "opus" } });
    expect(h).toContain("365");
    expect(h).not.toMatch(/\+\s*"model"/);
  });
});

describe("every file a category edits has its own path", () => {
  it("points Codex and Antigravity at their own files, not Marshell's", () => {
    expect(settingsPath("codex")).toContain(".codex");
    expect(settingsPath("antigravity")).toBe("C:\\Users\\Home Office\\.gemini\\antigravity-cli\\settings.json");
    expect(settingsPath("general")).toContain(".marshell");
  });
});

describe("a managed value", () => {
  it("shows the value the organisation enforces, and says the user's own value is not used", () => {
    const h = page();
    expect(h).toMatch(/<option value="plan" selected/);
    expect(h).toContain("Your organisation sets this to plan; your own value (default) is not used.");
  });
});

describe("an unknown value is an en dash, not a zero", () => {
  const field: SettingField = { category: "x", key: "n", label: "Count", help: "h", kind: "number", scope: "user", applies: "live" };
  it("shows an en dash placeholder instead of 0 for a number nobody set", () => {
    const h = renderToStaticMarkup(<FieldRow field={field} />);
    expect(h).toContain('placeholder="–"');
    expect(h).not.toContain('value="0"');
  });
  it("shows an en dash text control for a boolean it cannot read, never an off switch", () => {
    const bool: SettingField = { category: "x", key: "b", label: "Play sounds", help: "h", kind: "boolean", scope: "user", applies: "live" };
    const h = renderToStaticMarkup(<FieldRow field={bool} file={{ unparsable: true }} />);
    expect(h).not.toContain("switch");
    expect(h).toContain('value="–"');
    expect(h).toContain("data-unknown");
    expect(h).toMatch(/<label for="([^"]+)"[^>]*>Play sounds<\/label>/);
  });
});

describe("a conflict never offers to overwrite", () => {
  it("offers to review the difference and to reload, and has no Keep mine", () => {
    const h = page({ state: { conflict: true, changed: true } });
    expect(h).toContain(">Review the difference<");
    expect(h).toContain(">Reload<");
    expect(h).not.toContain("Keep mine");
  });
});

describe("the scope badge", () => {
  it("carries its explanation as text a screen reader reads, not only in a tooltip", () => {
    const h = renderToStaticMarkup(<ScopeBadge scope="managed" />);
    expect(h).toContain('class="scope-badge__note"');
    expect(h).toContain(SCOPE_NOTE.managed.replace(/'/g, "&#x27;"));
  });
});

describe("the unsafe-file tests really render the change strip", () => {
  it("read-only with pending changes: the strip is there and has no Apply", () => {
    const h = page({ state: { readOnly: true, changed: true, reviewing: true }, edits: { cleanupPeriodDays: 365 } });
    expect(h).toContain("change-bar");
    expect(h).not.toContain(">Apply<");
    expect(h).toContain(">Discard<");
  });
  it("a conflict with pending changes: the strip is there and has no Apply", () => {
    const h = page({ state: { conflict: true, changed: true, reviewing: true }, edits: { cleanupPeriodDays: 365 } });
    expect(h).toContain("change-bar");
    expect(h).not.toContain(">Apply<");
  });
});

describe("wording", () => {
  it("does not talk about phases in the Advanced empty state", () => {
    expect(page({ category: "advanced" })).not.toContain("later phase");
  });
});
