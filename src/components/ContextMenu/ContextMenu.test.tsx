import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ContextMenu, MenuList, type MenuItem } from "./ContextMenu";

const items: MenuItem[] = [
  { id: "links", label: "Open terminal links here", checked: true },
  { id: "auto", label: "Open HTML files agents write", checked: false, note: "Not in this session: Codex's hooks aren't set up." },
  { id: "plain", label: "Save screenshot" },
  {
    id: "viewport",
    label: "Viewport",
    submenu: true,
    rows: [
      { id: "viewport:responsive", label: "Responsive", checked: true, radio: true },
      { id: "viewport:phone", label: "Phone (390 px)", checked: false, radio: true },
    ],
  },
];

const noop = () => {};
const html = renderToStaticMarkup(<ContextMenu items={items} label="Browser" anchor={{ x: 0, y: 0 }} onRun={noop} onClose={noop} />);
/** The opening tag of the row whose text holds `label`. */
const row = (h: string, label: string) => {
  const at = h.indexOf(label);
  expect(at).toBeGreaterThan(-1);
  const start = h.lastIndexOf("<li", at);
  return h.slice(start, h.indexOf(">", start) + 1);
};

describe("ContextMenu rows", () => {
  it("draws a checked row as a checkbox item with its state", () => {
    expect(row(html, "Open terminal links here")).toContain('role="menuitemcheckbox"');
    expect(row(html, "Open terminal links here")).toContain('aria-checked="true"');
    expect(row(html, "Open HTML files agents write")).toContain('role="menuitemcheckbox"');
    expect(row(html, "Open HTML files agents write")).toContain('aria-checked="false"');
  });
  it("draws a row with no checked state as a plain item", () => {
    expect(row(html, "Save screenshot")).toContain('role="menuitem"');
    expect(row(html, "Save screenshot")).not.toContain("aria-checked");
  });
  it("shows a note under a row that still runs", () => {
    expect(html).toContain("Not in this session: Codex&#x27;s hooks aren&#x27;t set up.");
    expect(row(html, "Open HTML files agents write")).not.toContain("aria-disabled");
    expect(row(html, "Open HTML files agents write")).toContain("aria-describedby=");
  });
  it("points a row at its own note", () => {
    const tag = row(html, "Open HTML files agents write");
    const id = /aria-describedby="([^"]+)"/.exec(tag)?.[1];
    expect(id).toBeTruthy();
    expect(html).toContain(`id="${id}"`);
  });
  it("marks a row that opens a submenu, closed at first", () => {
    expect(row(html, "Viewport")).toContain('aria-haspopup="menu"');
    expect(row(html, "Viewport")).toContain('aria-expanded="false"');
    expect(html).not.toContain("Phone (390 px)");
  });
  it("leaves the marks off a menu with no checked row", () => {
    const plain = renderToStaticMarkup(<ContextMenu items={[{ id: "a", label: "Rename" }]} label="Row" anchor={{ x: 0, y: 0 }} onRun={noop} onClose={noop} />);
    expect(plain).not.toContain("context-menu__mark");
  });
  it("hides the mark from a screen reader", () => {
    expect(html).toContain("context-menu__mark");
    const svgs = html.match(/<svg[^>]*>/g) ?? [];
    expect(svgs.length).toBeGreaterThan(0);
    expect(svgs.every((s) => s.includes('aria-hidden="true"'))).toBe(true);
  });
});

describe("MenuList", () => {
  const sub = items[3] as Extract<MenuItem, { id: string }>;
  const list = renderToStaticMarkup(<MenuList items={sub.rows!} label="Viewport" active={0} openId={null} onActive={noop} onRun={noop} onOpen={noop} />);
  it("draws a radio row inside an open submenu", () => {
    expect(row(list, "Responsive")).toContain('role="menuitemradio"');
    expect(row(list, "Responsive")).toContain('aria-checked="true"');
    expect(row(list, "Phone (390 px)")).toContain('role="menuitemradio"');
    expect(row(list, "Phone (390 px)")).toContain('aria-checked="false"');
  });
  it("names the submenu by its row", () => {
    expect(list).toContain('role="menu"');
    expect(list).toContain('aria-label="Viewport"');
  });
});
