import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FilesSettings } from "../../generated/FilesSettings";
import { SETTINGS_SECTIONS } from "../settings/settingsSections";

const state: { settings: FilesSettings | null; failed: boolean; set: (c: Partial<FilesSettings>) => Promise<void> } = { settings: null, failed: false, set: () => Promise.resolve() };
vi.mock("./filesSettings", () => ({ useFilesSettings: () => ({ settings: state.settings, failed: state.failed, set: (c: Partial<FilesSettings>) => state.set(c) }) }));

import { FilesSection } from "./FilesSection";

const render = () => renderToStaticMarkup(<FilesSection announce={() => {}} />);
const switchOf = (html: string, label: string): string => {
  const at = html.indexOf(label);
  const start = html.lastIndexOf("<input", at);
  return html.slice(start, html.indexOf(">", start) + 1);
};
const radios = (html: string): string[] => html.match(/<input[^>]*type="radio"[^>]*>/g) ?? [];

beforeEach(() => {
  state.settings = { preview_tabs: true, auto_save: false, tab_indents: true, word_wrap: false, indent_guides: true, whitespace: false, scroll_past_end: false, tab_size: 4, tree_position: "automatic" };
  state.failed = false;
  state.set = () => Promise.resolve();
});

describe("FilesSection", () => {
  it("lists the nine settings with their lines", () => {
    const html = render();
    for (const t of [
      "Preview tabs",
      "A file opened from the tree shows in a tab that the next one replaces, until you edit it or double-click it.",
      "Auto save",
      "Saves a file 1 second after you stop typing and when you leave its tab. It stops when the file changed on disk.",
      "Tab key indents",
      "When off, Tab moves to the next control. Esc, then Tab, always leaves the editor.",
      "Word wrap",
      "When off, Markdown and plain text wrap and code doesn&#x27;t. When on, every file wraps. Alt+Z changes the tab in front.",
      "Indent guides",
      "Draws a line at each indent level.",
      "Show whitespace",
      "Draws spaces as dots and tabs as arrows.",
      "Scroll past end",
      "Lets the last line scroll up to the top.",
      "Tab size",
      "How wide a tab is drawn, and the indent of a file whose own can&#x27;t be told.",
      "File tree position",
      "Automatic",
      "At the window&#x27;s edge",
      "Left",
      "Right",
    ])
      expect(html).toContain(t);
    expect(radios(html)).toHaveLength(6);
  });
  it("shows the saved values", () => {
    const html = render();
    expect(switchOf(html, "Preview tabs")).toMatch(/checked=""/);
    expect(switchOf(html, "Auto save")).not.toContain("checked");
    expect(radios(html).map((x) => x.includes("checked"))).toEqual([false, true, false, true, false, false]);
  });
  it("shows nothing on until the settings are read", () => {
    state.settings = null;
    const html = render();
    expect(html).not.toContain("checked");
    expect(switchOf(html, "Preview tabs")).toContain("disabled");
    expect(html).not.toContain("Your settings couldn&#x27;t be read");
    state.failed = true;
    expect(render()).toContain("Your settings couldn&#x27;t be read");
  });
  it("is the section after Browser", () => {
    expect(SETTINGS_SECTIONS.map((s) => s.id).slice(0, 4)).toEqual(["appearance", "terminal", "browser", "files"]);
  });
  it("says when a save failed", async () => {
    state.set = () => Promise.reject(new Error("no"));
    const said: string[] = [];
    const { saveFilesSetting } = await import("./FilesSection");
    await saveFilesSetting(state.set, (t) => said.push(t), { auto_save: true }, "Auto save");
    expect(said).toEqual(["The setting wasn't saved."]);
  });
});
