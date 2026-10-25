import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PortChip } from "../../generated/PortChip";
import { PortChips } from "./PortChips";

const chip = (port: number): PortChip => ({ port, url: `http://localhost:${port}/`, announced: false });
const noMenu = () => {};

describe("PortChips", () => {
  it("renders a button per port with a name that says where it opens", () => {
    const h = renderToStaticMarkup(<PortChips ports={[chip(3000), chip(5173)]} onMenu={noMenu} />);
    expect(h).toContain('aria-label="Open http://localhost:3000/ in the browser"');
    expect(h).toContain('aria-label="Open http://localhost:5173/ in the browser"');
    expect(h).toContain("localhost:5173");
    expect(h.match(/<button/g)).toHaveLength(2);
    expect(h).not.toContain("+");
  });
  it("shows at most max chips and a +N chip for the rest", () => {
    const h = renderToStaticMarkup(<PortChips ports={[chip(1), chip(2), chip(3)]} max={2} onMenu={noMenu} />);
    expect(h).toContain("+1");
    expect(h).toContain('aria-haspopup="menu"');
    expect(h).not.toContain("localhost:3<");
    expect(h).not.toContain("http://localhost:3/");
  });
  it("renders nothing for no ports", () => {
    expect(renderToStaticMarkup(<PortChips ports={[]} onMenu={noMenu} />)).toBe("");
  });
});
