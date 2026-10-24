import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { HistorySettingsDialogView } from "./HistorySettingsDialog";

const none = () => {};
const view = (on: boolean | null, error: string | null = null) => renderToStaticMarkup(<HistorySettingsDialogView on={on} error={error} onChange={none} onClose={none} />);

describe("HistorySettingsDialogView", () => {
  it("is a modal dialog titled History settings", () => {
    const h = view(true);
    expect(h).toMatch(/role="dialog"[^>]*aria-modal="true"[^>]*aria-label="History settings"/);
    expect(h).toContain(">History settings</h2>");
  });
  it("has the switch and the hint, word for word", () => {
    const h = view(true);
    expect(h).toContain('<span class="switch__text">Copy conversations run outside Marshell</span>');
    expect(h).toContain("Marshell reads Claude Code&#x27;s and Codex&#x27;s conversation folders and keeps its own copy of each conversation. It never changes their files.");
  });
  it("shows the switch checked or not from the setting", () => {
    expect(view(true)).toMatch(/<input[^>]*role="switch"[^>]*checked=""/);
    expect(view(false)).not.toMatch(/<input[^>]*role="switch"[^>]*checked=""/);
  });
  it("shows no switch until the setting has been read", () => {
    expect(view(null)).not.toContain('role="switch"');
  });
  it("shows the Storage section under the switch, and none before the setting is read", () => {
    const h = view(true);
    expect(h.indexOf('role="switch"')).toBeGreaterThan(-1);
    expect(h.indexOf(">Storage</h3>")).toBeGreaterThan(h.indexOf('role="switch"'));
    expect(view(null)).not.toContain(">Storage</h3>");
  });
});
