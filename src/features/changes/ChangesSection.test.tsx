import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChangesSettings } from "../../generated/ChangesSettings";
import { SETTINGS_SECTIONS } from "../settings/settingsSections";

const state: { settings: ChangesSettings | null; failed: boolean; set: (c: Partial<ChangesSettings>) => Promise<void> } = { settings: null, failed: false, set: () => Promise.resolve() };
vi.mock("./changesSettings", () => ({ useChangesSettings: () => ({ settings: state.settings, failed: state.failed, set: (c: Partial<ChangesSettings>) => state.set(c) }) }));

import { ChangesSection } from "./ChangesSection";

const render = () => renderToStaticMarkup(<ChangesSection announce={() => {}} />);
const switchOf = (html: string, label: string): string => {
  const at = html.indexOf(label);
  const start = html.lastIndexOf("<input", at);
  return html.slice(start, html.indexOf(">", start) + 1);
};

beforeEach(() => {
  state.settings = { hide_whitespace: false, wrap: true, pull_requests: true };
  state.failed = false;
  state.set = () => Promise.resolve();
});

describe("ChangesSection", () => {
  it("names both switches and shows the saved values", () => {
    const html = render();
    expect(html).toContain("Hide whitespace changes");
    expect(html).toContain("Wrap long lines");
    expect(switchOf(html, "Hide whitespace changes")).not.toContain("checked");
    expect(switchOf(html, "Wrap long lines")).toMatch(/checked=""/);
  });
  it("shows the GitHub switch with its help line", () => {
    const html = render();
    expect(html).toContain("Show pull requests from GitHub");
    expect(switchOf(html, "Show pull requests from GitHub")).toMatch(/checked=""/);
    expect(html).toContain("Asks GitHub through the GitHub CLI while Changes shows a checkout whose origin is on GitHub. Off, Marshell never runs gh.");
    state.settings = { hide_whitespace: false, wrap: true, pull_requests: false };
    expect(switchOf(render(), "Show pull requests from GitHub")).not.toContain("checked");
  });
  it("shows nothing on until the settings are read", () => {
    state.settings = null;
    const html = render();
    expect(html).not.toContain("checked");
    expect(switchOf(html, "Wrap long lines")).toContain("disabled");
    expect(html).not.toContain("Your settings couldn&#x27;t be read");
    state.failed = true;
    expect(render()).toContain("Your settings couldn&#x27;t be read");
  });
  it("is the section after Files", () => {
    expect(SETTINGS_SECTIONS.map((s) => s.id).slice(3, 6)).toEqual(["files", "changes", "notifications"]);
    expect(SETTINGS_SECTIONS.find((s) => s.id === "changes")?.label).toBe("Changes");
  });
  it("says when a save failed", async () => {
    const said: string[] = [];
    const { saveChangesSetting } = await import("./ChangesSection");
    await saveChangesSetting(() => Promise.reject(new Error("no")), (t) => said.push(t), { wrap: false }, "Wrap long lines");
    expect(said).toEqual(["The setting wasn't saved."]);
  });
});
