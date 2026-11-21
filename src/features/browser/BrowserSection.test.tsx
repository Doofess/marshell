import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BrowserSettings } from "../../generated/BrowserSettings";
import { SETTINGS_SECTIONS } from "../settings/settingsSections";

const state: { settings: BrowserSettings | null; failed: boolean } = { settings: null, failed: false };
vi.mock("./browserSettings", () => ({ useBrowserSettings: () => ({ settings: state.settings, failed: state.failed, set: () => Promise.resolve() }) }));

import { BrowserSection } from "./BrowserSection";

const render = (props: Partial<Parameters<typeof BrowserSection>[0]> = {}) => renderToStaticMarkup(<BrowserSection announce={() => {}} pages platform="windows" found={["claude", "codex"]} hooks={{ claude: "installed" }} {...props} />);

const switchOf = (html: string, label: string): string => {
  const at = html.indexOf(label);
  const start = html.lastIndexOf("<input", at);
  return html.slice(start, html.indexOf(">", start) + 1);
};
const radios = (html: string): string[] => html.match(/<input[^>]*type="radio"[^>]*>/g) ?? [];

beforeEach(() => {
  state.settings = { terminal_links: true, open_agent_html: false, cookies: "until-quit" };
  state.failed = false;
});

describe("BrowserSection", () => {
  it("shows the two switches with their state and lines", () => {
    const html = render();
    expect(html).toContain("Ctrl+click on a web link in terminal output opens it in the Browser panel. When this is off, it opens your system browser.");
    expect(html).toContain("An HTML file an agent creates or edits opens as a background tab in its session&#x27;s Browser panel. A file written by a shell command isn&#x27;t seen. In a WSL session, only files on a Windows drive (under /mnt) open.");
    expect(switchOf(html, "Open terminal links here")).toMatch(/role="switch"[^>]*checked=""|checked=""[^>]*role="switch"/);
    expect(switchOf(html, "Open HTML files agents write")).not.toContain("checked");
  });
  it("says Cmd on macOS and leaves the WSL sentence to Windows", () => {
    const html = render({ platform: "macos" });
    expect(html).toContain("⌘+click on a web link");
    expect(html).not.toContain("WSL");
  });
  it("lists each CLI found with what it does", () => {
    const html = render();
    expect(html).toContain("Claude Code: Opens its HTML files");
    expect(html).toContain("Codex: Its hooks aren&#x27;t set up");
    expect(render({ found: [] })).toContain("No agent CLI was found.");
  });
  it("offers the three cookie rules with the saved one chosen", () => {
    const html = render();
    const r = radios(html);
    expect(r).toHaveLength(3);
    expect(r.map((x) => x.includes("checked"))).toEqual([false, true, false]);
    for (const t of ["Keep always", "Keep until quit", "Never keep", "Sign-ins last until you clear browsing data.", "Cookies and site storage are cleared when Marshell quits and when it starts.", "Every Browser tab is a private page. Nothing is written to disk, and it is gone when the last Browser tab closes."]) expect(html).toContain(t);
  });
  it("turns the page-only settings off where there are no pages", () => {
    const html = render({ pages: false });
    expect(switchOf(html, "Open terminal links here")).toContain("disabled");
    expect(switchOf(html, "Open terminal links here")).not.toContain("checked");
    expect(switchOf(html, "Open HTML files agents write")).not.toContain("disabled");
    expect(radios(html).every((x) => x.includes("disabled"))).toBe(true);
    expect(html).toContain("Not available on this system yet");
  });
  it("shows nothing as set until the settings are read", () => {
    state.settings = null;
    const html = render();
    expect(html).not.toContain("checked");
    expect(switchOf(html, "Open terminal links here")).toContain("disabled");
    expect(switchOf(html, "Open HTML files agents write")).toContain("disabled");
    expect(radios(html).every((x) => x.includes("disabled"))).toBe(true);
    expect(html).not.toContain("Your settings couldn&#x27;t be read");
    state.failed = true;
    expect(render()).toContain("Your settings couldn&#x27;t be read");
  });
  it("is the section after Terminal", () => {
    expect(SETTINGS_SECTIONS.map((s) => s.id).slice(0, 3)).toEqual(["appearance", "terminal", "browser"]);
  });
});
