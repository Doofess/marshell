import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { SettingsPage } from "./SettingsPage";
import { SETTINGS_TEXT, SETTINGS_TEXT_BROKEN, SETTINGS_TEXT_LONG } from "./fixtures";

/** renderToStaticMarkup writes an apostrophe as &#x27;; the visible text is the same. */
const esc = (s: string) => s.replace(/'/g, "&#x27;");

const base = { category: "claude", mode: "form" as const, state: {} };
describe("SettingsPage: form mode", () => {
  it("shows every field with its label, its scope badge, what it affects and when", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toContain("Delete transcripts after (days)");
    expect(h).toContain('data-scope="user"');
    expect(h).toContain("Applies to new sessions.");
  });
  it("locks a managed field, says who set it and gives it no editable control", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toContain(esc("Set by your organisation. Marshell can't change it."));
    expect(h).toMatch(/data-locked="true"[\s\S]*?disabled/);
  });
  it("marks the selected category with aria-current, in a nav", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toMatch(/<nav[^>]*aria-label="Settings sections"/);
    expect(h).toMatch(/aria-current="true"[^>]*>[\s\S]*?Claude Code/);
  });
  it("has the Edit as radio group with Form and Raw", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toContain('role="radiogroup"');
    expect(h).toContain("Edit as");
    expect(h.match(/type="radio"/g)).toHaveLength(2);
  });
  it("is a level 1 heading in the app, and can sit lower when it is shown among other headings", () => {
    expect(renderToStaticMarkup(<SettingsPage {...base} />)).toMatch(/<h1[^>]*>Settings<\/h1>/);
    expect(renderToStaticMarkup(<SettingsPage {...base} level={2} />)).toMatch(/<h2[^>]*>Settings<\/h2>/);
  });
  it("shows the scope a value is really read from: a value a managed file locks says Managed, not User", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toMatch(/data-locked="true"[\s\S]*?data-scope="managed"/);
  });
  it("gives each page its own radio group name, so two pages in one document never share a choice", () => {
    const h = renderToStaticMarkup(
      <>
        <SettingsPage {...base} />
        <SettingsPage {...base} mode="raw" />
      </>,
    );
    const names = [...h.matchAll(/type="radio" name="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(names).size).toBe(2);
  });
  it("says an empty category is empty, with a way forward", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} category="codex" />);
    expect(h).toContain("No coding agents found");
  });
});

describe("SettingsPage: raw mode", () => {
  it("shows the file text in the editor instead of the form", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} mode="raw" opened={SETTINGS_TEXT} />);
    expect(h).toContain("<textarea");
    expect(h).not.toContain("Delete transcripts after");
  });
});

describe("SettingsPage: changes", () => {
  it("counts the changes, then shows the exact diff, the backup and the way to undo before Apply", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ cleanupPeriodDays: 365 }} opened={SETTINGS_TEXT} />);
    expect(h).toContain("1 change");
    expect(h).toContain("diff__lines");
    expect(h).toContain("A backup is saved first");
    expect(h).toContain("Settings → Advanced → Snapshots");
    expect(h).toContain(">Apply<");
    expect(h).toContain(">Discard<");
  });
  it("keeps a 200-line change inside the page (collapsed), not the window", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ hooks: Array.from({ length: 200 }, (_, i) => `h${i}`) }} opened={SETTINGS_TEXT_LONG} />);
    expect(h).toContain("Show all");
  });
  it("has no Apply when nothing changed", () => {
    expect(renderToStaticMarkup(<SettingsPage {...base} />)).not.toContain(">Apply<");
  });
  it("offers Review changes first, before the diff", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true }} edits={{ cleanupPeriodDays: 365 }} opened={SETTINGS_TEXT} />);
    expect(h).toContain(">Review changes<");
    expect(h).not.toContain("diff__lines");
  });
});

describe("SettingsPage: unsafe files", () => {
  it("read-only: says so and disables editing", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ readOnly: true }} />);
    expect(h).toContain(esc("This file is read-only, so Marshell can't save changes."));
    expect(h).not.toContain(">Apply<");
  });
  it("unparsable: fails closed, names the line, and leaves the form off", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ unparsable: true }} opened={SETTINGS_TEXT_BROKEN} mode="raw" />);
    expect(h).toContain(esc("This file does not parse, so Marshell won't edit it."));
    expect(h).toMatch(/Line 3/);
    expect(h).not.toContain(">Apply<");
  });
  it("conflict: says the file changed on disk and offers three ways forward", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ conflict: true, changed: true }} />);
    expect(h).toContain("The file changed on disk since you opened it.");
    for (const b of ["Show differences", "Keep mine", "Reload"]) expect(h).toContain(`>${b}<`);
    expect(h).not.toContain(">Apply<");
  });
});

describe("SettingsPage: accessibility", () => {
  it("has no live region of its own and no banned words", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ cleanupPeriodDays: 365 }} opened={SETTINGS_TEXT} />);
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
    expect(h).not.toMatch(/successfully|oops|!/i);
  });
  it("names its buttons from the text they show", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ cleanupPeriodDays: 365 }} opened={SETTINGS_TEXT} />);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
});
