import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { SCENARIO_PLAN, SCENARIO_PORTS } from "../window/scenario";
import { StatusBar } from "./StatusBar";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;
const css = readFileSync(new URL("./StatusBar.css", import.meta.url), "utf8");
const html = (id: string | null, width?: number) =>
  renderToStaticMarkup(<StatusBar row={id ? row(id) : null} plan={SCENARIO_PLAN} ports={SCENARIO_PORTS} width={width} />);
const items = (h: string) => (h.match(/class="status-strip__item"/g) ?? []).length;

describe("StatusBar: the focused session's facts, in one row at the foot of the window", () => {
  it("renders the strip for the session, inside the named group", () => {
    const h = html("working");
    expect(h).toContain('class="status-bar"');
    expect(h).toContain('role="group"');
    expect(h).toContain('aria-label="Session status"');
    expect(h).toContain("5h");
    expect(h).toContain("38%");
    expect(h).toContain(":3000");
  });
  it("carries the vendor of the focused session, so its meters take that colour", () => {
    expect(html("working")).toContain('data-agent="claude"');
    expect(html("idle")).toContain('data-agent="codex"');
  });
  it("is exactly one row: a single strip and nothing else but the host", () => {
    const h = html("working");
    expect((h.match(/class="status-strip"/g) ?? []).length).toBe(1);
    expect(h).not.toContain("<form");
    expect(h).not.toContain("<textarea");
    expect(h).not.toContain("<button");
  });
  it("is an empty bar of the same host when the focused pane is empty, so the window never changes height", () => {
    const h = html(null);
    expect(h).toContain('class="status-bar"');
    expect(h).not.toContain("status-strip__item");
    expect(h).not.toContain("data-agent");
  });
  it("leaves out what is unknown rather than showing dashes", () => {
    const h = renderToStaticMarkup(<StatusBar row={{ id: "x", name: "x", project: "p", agent: "claude", status: "idle", phrase: "Idle" }} />);
    expect(h).not.toContain("status-strip__item");
    expect(h.replace(/<[^>]+>/g, " ")).not.toMatch(/–|--/);
  });
  it("hides items by priority as it narrows: fitted in script when given a width, fewer items the narrower", () => {
    const wide = items(html("working", 220));
    const mid = items(html("working", 90));
    const narrow = items(html("working", 40));
    expect(wide).toBeGreaterThan(mid);
    expect(mid).toBeGreaterThan(narrow);
    expect(narrow).toBeGreaterThanOrEqual(1);
  });
  it("keeps the core items longest: the model, context and the 5-hour limit survive when the rest are gone", () => {
    const h = html("working", 60);
    for (const id of ["model", "ctx", "limit5"]) expect(h, id).toContain(`data-id="${id}"`);
    expect(h).not.toContain('data-id="venv"');
  });
  it("puts a mcp problem first when room is short", () => {
    expect(html("needs-permission", 50)).toContain('data-id="mcp"');
  });
});

describe("StatusBar.css", () => {
  const rule = (s: string) => {
    const i = css.indexOf(`${s} {`);
    return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
  };
  it("is the container the strip's width rules measure, so the stylesheet hides items by the bar's own width", () => {
    expect(rule(".status-bar")).toContain("container-type: inline-size");
  });
  it("is one row of its own height, raised a step from the terminal, with a hairline edge drawn as an inset shadow", () => {
    const r = rule(".status-bar");
    expect(r).toMatch(/block-size:\s*24px/);
    expect(r).toContain("background: var(--bg-raised)");
    expect(r).toContain("box-shadow: inset 0 1px 0 var(--hairline)");
  });
  it("is not a card: no radius, no outline, no border", () => {
    const r = rule(".status-bar");
    expect(r).not.toContain("border-radius");
    expect(r).not.toMatch(/border:|outline:/);
  });
  it("keeps its top edge in Windows high contrast", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)[\s\S]*outline: 1px solid CanvasText/);
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
  });
});
