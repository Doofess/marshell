import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { AllSessionsMenu } from "./AllSessionsMenu";

const twenty = Array.from({ length: 20 }, (_, i) => ({ ...FIXTURES[i % 5]!, id: `s${i}`, name: `session-${i + 1}` }));
const html = (p: Partial<Parameters<typeof AllSessionsMenu>[0]> = {}) => renderToStaticMarkup(<AllSessionsMenu rows={twenty} {...p} />);
const css = readFileSync(new URL("./AllSessionsMenu.css", import.meta.url), "utf8");

describe("AllSessionsMenu", () => {
  it("is a named dialog with every session in it, however many there are", () => {
    const h = html();
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-label="All sessions"');
    expect((h.match(/class="all-sessions__row"/g) ?? []).length).toBe(20);
  });
  it("takes its own title, and counts what it holds", () => {
    const h = html({ title: "Needs you" });
    expect(h).toContain('aria-label="Needs you"');
    expect(h).toContain("20 sessions");
  });
  it("shows each session's status glyph, name, project and phrase, and names the button with its visible text first", () => {
    const h = html();
    expect(h).toContain("status-glyph");
    expect(h).toContain("session-1");
    expect(h).toMatch(/aria-label="session-1 /);
  });
  it("marks the selected session", () => {
    expect(html({ selectedId: "s3" })).toContain('aria-current="true"');
    expect(html()).not.toContain("aria-current");
  });
  it("carries the vendor on each row so the focus ring and tint follow it", () => {
    expect(html()).toContain('data-agent="claude"');
  });
  it("is the one place in the main window's shell where a list may scroll, with the themed scrollbar visible", () => {
    expect(html()).toContain("scroll-menu");
    expect(css).toMatch(/\.scroll-menu\s*\{[^}]*overflow-y:\s*auto/);
    expect(css).toMatch(/max-block-size/);
    expect(css).not.toMatch(/scrollbar-width:\s*none|::-webkit-scrollbar\s*\{\s*display:\s*none/);
  });
  it("uses fills, tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).toContain(".all-sessions__row:focus-visible");
    expect(css).toContain(".all-sessions__row:active");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("puts no literal colour in the markup", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});

describe("AllSessionsMenu: names and text agree", () => {
  it("puts a space between the name, project and phrase, so the visible text is contained in the accessible name", () => {
    const h = renderToStaticMarkup(<AllSessionsMenu rows={FIXTURES.slice(0, 3)} />);
    expect(h).toMatch(/<\/span> <span class="all-sessions__project"/);
    expect(h).toMatch(/<\/span> <span class="all-sessions__phrase"/);
  });
});
