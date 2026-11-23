import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EmptyBrowser } from "./EmptyBrowser";
import { EmptyFiles } from "./EmptyFiles";
import { EmptyPanel } from "./EmptyPanel";

const noop = () => {};
const browser = (available: boolean, ports: { port: number; url: string }[]) =>
  renderToStaticMarkup(<EmptyBrowser available={available} ports={ports} onNewTab={noop} onOpenHtml={noop} onPort={noop} />);
const files = (recent: string[]) => renderToStaticMarkup(<EmptyFiles recent={recent} onOpenFile={noop} onBrowse={noop} onRecent={noop} />);
/** Each `<button …>…</button>` of the markup, in order. */
const buttons = (out: string) => [...out.matchAll(/<button\b[^>]*>.*?<\/button>/g)].map((m) => m[0]);
const PORTS = [
  { port: 5173, url: "http://localhost:5173/" },
  { port: 3000, url: "http://localhost:3000/" },
];

describe("EmptyBrowser", () => {
  it("the Browser block says there are no pages and offers New tab and Open HTML file…", () => {
    const out = browser(true, PORTS);
    expect(out).toContain('<h2 class="empty-panel__title">No pages open</h2>');
    expect(out).toContain("Open an address, an HTML file or a server this session runs.");
    const [first, second] = buttons(out);
    expect(first).toContain("New tab");
    expect(first).toContain("empty-panel__button--primary");
    expect(second).toContain("Open HTML file…");
    expect(second).not.toContain("empty-panel__button--primary");
    expect(out).not.toContain("aria-disabled");
  });
  it("lists the session's ports as buttons, in order", () => {
    const out = browser(true, PORTS);
    expect(out).toContain('<h3 class="empty-panel__heading">Running in this session</h3>');
    const rows = buttons(out).filter((b) => b.includes("empty-panel__row"));
    expect(rows).toHaveLength(2);
    expect(rows[0]).toContain("localhost:5173");
    expect(rows[1]).toContain("localhost:3000");
  });
  it("leaves the ports row out and changes its line when the session listens on nothing", () => {
    const out = browser(true, []);
    expect(out).not.toContain("Running in this session");
    expect(out).not.toContain("empty-panel__row");
    expect(out).toContain("Open an address or an HTML file.");
    expect(out).not.toContain("a server this session runs");
  });
  it("where pages aren't available Open HTML file… leads, New tab is aria-disabled with its reason, and no port is listed", () => {
    const out = browser(false, PORTS);
    expect(out).toContain("Open an HTML file. Web pages aren&#x27;t available on this system yet.");
    const [first, second] = buttons(out);
    expect(first).toContain("Open HTML file…");
    expect(first).toContain("empty-panel__button--primary");
    expect(second).toContain("New tab");
    expect(second).toContain('aria-disabled="true"');
    expect(second).toContain('title="Not available on this system yet"');
    expect(second).not.toContain(" disabled");
    expect(out).not.toContain("Running in this session");
    expect(out).not.toContain("localhost");
  });
});

describe("EmptyFiles", () => {
  it("the Files block says there are no files and offers Go to file… and Browse…", () => {
    const out = files([]);
    expect(out).toContain('<h2 class="empty-panel__title">No files open</h2>');
    expect(out).toContain("Open a file from this project, or drop one here.");
    const [first, second] = buttons(out);
    expect(first).toContain("Go to file…");
    expect(first).toContain("empty-panel__button--primary");
    expect(second).toContain("Browse…");
    expect(second).not.toContain("empty-panel__button--primary");
  });
  it("lists recent files by name and folder under Recent in this project", () => {
    const out = files(["C:\\work\\app\\README.md"]);
    expect(out).toContain('<h3 class="empty-panel__heading">Recent in this project</h3>');
    expect(out).toContain(">README.md<");
    expect(out).toContain(">C:\\work\\app<");
    expect(out).toContain('title="C:\\work\\app\\README.md"');
  });
  it("shows no heading with no recent files", () => {
    expect(files([])).not.toContain("Recent in this project");
  });
});

describe("the empty panels", () => {
  it("each block's icon is hidden from a screen reader", () => {
    for (const out of [browser(true, PORTS), browser(false, []), files(["/a/b.md"])]) {
      const svgs = [...out.matchAll(/<svg\b[^>]*>/g)].map((m) => m[0]);
      expect(svgs.length).toBeGreaterThan(0);
      for (const svg of svgs) expect(svg).toContain('aria-hidden="true"');
    }
  });
  it("has no tabindex of its own, and every control is a button", () => {
    for (const out of [browser(true, PORTS), files(["/a/b.md"])]) {
      expect(out).not.toContain("tabindex");
      expect(out).not.toContain("<a ");
    }
  });
});

describe("EmptyPanel actions", () => {
  const act = (label: string) => ({ label, icon: null, onClick: noop });
  it("shows two actions in order", () => {
    const out = renderToStaticMarkup(<EmptyPanel icon={null} title="T" line="L" actions={[act("One"), act("Two")]} />);
    const [a, b] = buttons(out);
    expect(a).toContain("One");
    expect(a).toContain("empty-panel__button--primary");
    expect(b).toContain("Two");
  });
  it("shows one action, or none", () => {
    expect(buttons(renderToStaticMarkup(<EmptyPanel icon={null} title="T" line="L" actions={[act("One")]} />))).toHaveLength(1);
    expect(buttons(renderToStaticMarkup(<EmptyPanel icon={null} title="T" line="L" actions={[]} />))).toHaveLength(0);
  });
});
