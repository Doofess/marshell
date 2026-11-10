import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { UrlBar } from "./UrlBar";
import type { PageState } from "./pageStore";

const noop = () => {};
const page = (more: Partial<PageState> = {}): PageState => ({
  url: "https://example.com/docs",
  title: "Docs",
  canBack: false,
  canForward: false,
  loading: false,
  icon: null,
  focused: false,
  status: null,
  live: true,
  ...more,
});
const html = (p: PageState | null = page()) => renderToStaticMarkup(<UrlBar page={p ?? undefined} onGo={noop} onNav={noop} onOpenOutside={noop} onEscape={noop} />);
const buttons = (out: string) => [...out.matchAll(/<button\b[^>]*>[\s\S]*?<\/button>/g)].map((m) => m[0]);
const button = (out: string, label: string) => buttons(out).find((b) => b.includes(`aria-label="${label}"`))!;

describe("UrlBar", () => {
  it("is the page's toolbar, with four icon buttons with labels and titles", () => {
    const out = html();
    expect(out).toContain('role="toolbar"');
    expect(out).toContain('aria-label="Page"');
    expect(buttons(out)).toHaveLength(4);
    for (const label of ["Back", "Forward", "Reload", "Open in the system browser"]) {
      expect(button(out, label), label).toContain(`title="${label}"`);
    }
  });
  it("puts Back, Forward and Reload before the box, and the system browser after it", () => {
    const out = html();
    const at = (s: string) => out.indexOf(s);
    expect(at('aria-label="Back"')).toBeLessThan(at('aria-label="Forward"'));
    expect(at('aria-label="Forward"')).toBeLessThan(at('aria-label="Reload"'));
    expect(at('aria-label="Reload"')).toBeLessThan(at('aria-label="Address"'));
    expect(at('aria-label="Address"')).toBeLessThan(at('aria-label="Open in the system browser"'));
  });
  it("Back and Forward follow the page's history", () => {
    expect(button(html(page()), "Back")).toContain("disabled");
    expect(button(html(page()), "Forward")).toContain("disabled");
    expect(button(html(page({ canBack: true })), "Back")).not.toContain("disabled");
    expect(button(html(page({ canBack: true })), "Forward")).toContain("disabled");
    expect(button(html(page({ canForward: true })), "Forward")).not.toContain("disabled");
    expect(button(html(null), "Back")).toContain("disabled");
  });
  it("Reload reads Stop while the page loads", () => {
    expect(html(page({ loading: true }))).toContain('aria-label="Stop"');
    expect(html(page({ loading: true }))).toContain('title="Stop"');
    expect(html(page({ loading: true }))).not.toContain('aria-label="Reload"');
    expect(html(page({ loading: false }))).not.toContain('aria-label="Stop"');
  });
  it("open outside is disabled on a blank page, and with no page address", () => {
    expect(button(html(page({ url: "about:blank" })), "Open in the system browser")).toContain("disabled");
    expect(button(html(null), "Open in the system browser")).toContain("disabled");
    expect(button(html(page()), "Open in the system browser")).not.toContain("disabled");
  });
  it("the box shows the page's address", () => {
    const out = html();
    expect(out).toMatch(/<input[^>]*aria-label="Address"/);
    expect(out).toContain('value="https://example.com/docs"');
  });
  it("shows the saved address while the page isn't loaded", () => {
    const out = renderToStaticMarkup(<UrlBar page={undefined} fallbackUrl="https://saved.example/" onGo={noop} onNav={noop} onOpenOutside={noop} onEscape={noop} />);
    expect(out).toContain('value="https://saved.example/"');
    expect(button(out, "Open in the system browser")).not.toContain("disabled");
  });
  it("leaves the box empty on a blank page, so the placeholder reads", () => {
    expect(html(page({ url: "about:blank" }))).toContain('value=""');
  });
  it('the placeholder reads "Search Google or type an address"', () => {
    expect(html()).toContain('placeholder="Search Google or type an address"');
  });
  it("no button has visible text", () => {
    for (const b of buttons(html(page({ canBack: true, loading: true })))) {
      expect(b).toMatch(/<svg[^>]*aria-hidden="true"/);
      expect(b.replace(/<svg[\s\S]*?<\/svg>/g, "").replace(/<[^>]*>/g, "")).toBe("");
    }
  });
  it("shows the status line for a refused or a failed page, in the plan's words", () => {
    expect(html(page({ status: "refused" }))).toContain("That address can&#x27;t be opened in the side pane.");
    expect(html(page({ status: "failed" }))).toContain("This page stopped working. Reload to try again.");
    expect(html()).not.toContain("url-bar__status");
  });
  it("draws the status line as plain text, not a live region", () => {
    expect(html(page({ status: "refused" }))).not.toMatch(/role="(?:status|alert)"|aria-live/);
  });
});
