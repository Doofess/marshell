import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BrowserTabView, goTo } from "./BrowserTab";
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
  live: true,
  ...more,
});
type Props = Partial<Parameters<typeof BrowserTabView>[0]>;
const html = (more: Props = {}) =>
  renderToStaticMarkup(<BrowserTabView page={page()} url="https://example.com/" available keys={false} frozen={null} slotRef={() => {}} onGo={noop} onNav={noop} onOpenOutside={noop} onEscape={noop} {...more} />);

describe("BrowserTabView", () => {
  it('has a focusable slot labelled "Web page"', () => {
    const out = html();
    const slot = out.match(/<div[^>]*class="browser-tab__slot"[^>]*>/)![0];
    expect(slot).toContain('tabindex="0"');
    expect(slot).toContain('aria-label="Web page"');
  });
  it("names its page on the slot, so the app finds the slot a page's keys and focus belong to", () => {
    const slot = html({ pageKey: "s1/p2" }).match(/<div[^>]*class="browser-tab__slot"[^>]*>/)![0];
    expect(slot).toContain('data-page-key="s1/p2"');
    expect(html()).not.toContain("data-page-key");
  });
  it("the slot says when its page has the keys", () => {
    const slot = (out: string) => out.match(/<div[^>]*class="browser-tab__slot"[^>]*>/)![0];
    expect(slot(html({ keys: true }))).toContain('data-keys=""');
    expect(slot(html({ keys: false }))).not.toContain("data-keys");
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

// Enter in the address box: the page goes there and the keyboard follows it, as in any browser (found in the 6q-3a live check).
describe("goTo", () => {
  const api = (calls: string[], refuse = false) => ({
    nav: async (key: string, action: string, url?: string) => {
      calls.push(`nav ${key} ${action} ${url}`);
      if (refuse) throw new Error("refused");
    },
    open: async (key: string, url: string) => void calls.push(`open ${key} ${url}`),
    focus: async (key: string, reason: string) => void calls.push(`focus ${key} ${reason}`),
  });
  const slot = (calls: string[]) => () => void calls.push("slot");

  it("sends the page to the address, then moves the keyboard into it", async () => {
    const calls: string[] = [];
    await goTo(api(calls), "s/t", true, "https://a.test/", () => true, slot(calls));
    expect(calls).toEqual(["nav s/t go https://a.test/", "slot", "focus s/t programmatic"]);
  });
  it("opens a page that is gone at the address, then moves the keyboard into it", async () => {
    const calls: string[] = [];
    await goTo(api(calls), "s/t", false, "https://a.test/", () => true, slot(calls));
    expect(calls).toEqual(["open s/t https://a.test/", "slot", "focus s/t programmatic"]);
  });
  it("leaves the keyboard in the address box when the shell refuses the address", async () => {
    const calls: string[] = [];
    await goTo(api(calls, true), "s/t", true, "http://tauri.localhost/", () => true, slot(calls));
    expect(calls).toEqual(["nav s/t go http://tauri.localhost/"]);
  });
  it("leaves the keyboard where the person moved it while the page was asked", async () => {
    const calls: string[] = [];
    await goTo(api(calls), "s/t", true, "https://a.test/", () => false, slot(calls));
    expect(calls).toEqual(["nav s/t go https://a.test/"]);
  });
});
