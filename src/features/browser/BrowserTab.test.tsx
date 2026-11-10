import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BrowserTabView } from "./BrowserTab";
import type { PageState } from "./pageStore";

const noop = () => {};
const page = (more: Partial<PageState> = {}): PageState => ({
  url: "https://example.com/",
  title: "Example",
  canBack: false,
  canForward: false,
  loading: false,
  icon: null,
  focused: false,
  status: null,
  ...more,
});
type Props = Partial<Parameters<typeof BrowserTabView>[0]>;
const html = (more: Props = {}) =>
  renderToStaticMarkup(<BrowserTabView page={page()} url="https://example.com/" available frozen={null} slotRef={() => {}} onGo={noop} onNav={noop} onOpenOutside={noop} onEscape={noop} {...more} />);

describe("BrowserTabView", () => {
  it('has a focusable slot labelled "Web page"', () => {
    const out = html();
    const slot = out.match(/<div[^>]*class="browser-tab__slot"[^>]*>/)![0];
    expect(slot).toContain('tabindex="0"');
    expect(slot).toContain('aria-label="Web page"');
  });
  it("a refused address shows the status line", () => {
    expect(html({ page: page({ status: "refused" }) })).toContain("That address can&#x27;t be opened in the side pane.");
  });
  it('a failed page shows "This page stopped working. Reload to try again."', () => {
    expect(html({ page: page({ status: "failed" }) })).toContain("This page stopped working. Reload to try again.");
  });
  it("where browser tabs aren't available the body says so and offers the system browser", () => {
    const out = html({ available: false, page: undefined });
    expect(out).toContain("Browser tabs aren&#x27;t available on this system yet.");
    expect(out).toMatch(/<button[^>]*>Open in the system browser<\/button>/);
    expect(out).not.toContain("browser-tab__slot");
    expect(out).not.toContain('aria-label="Address"');
  });
  it("holds the picture while frozen, and a plain fill when the capture failed", () => {
    const picture = html({ frozen: { picture: "data:image/png;base64,AAAA" } });
    expect(picture).toContain('<img alt=""');
    expect(picture).toContain('src="data:image/png;base64,AAAA"');
    const none = html({ frozen: { picture: null } });
    expect(none).not.toContain("<img");
    expect(none).toContain("browser-tab__fill");
    expect(html()).not.toContain("<img");
  });
});
