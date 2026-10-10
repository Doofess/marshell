import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { SCENARIO_PLAN, SCENARIO_PORTS } from "../window/scenario";
import { StatusStrip } from "./StatusStrip";
import { statusItems, stripWidth, fitItems } from "./stripItems";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;
const html = (id = "working", width?: number) => renderToStaticMarkup(<StatusStrip row={row(id)} plan={SCENARIO_PLAN} ports={SCENARIO_PORTS} width={width} />);
const count = (h: string, re: RegExp) => (h.match(re) ?? []).length;
const text = (h: string) => h.replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim();

describe("StatusStrip", () => {
  it("is one named group of the session's facts", () => {
    const h = html();
    expect(h).toContain('role="group"');
    expect(h).toContain('aria-label="Session status"');
    expect(count(h, /class="status-strip__item"/g)).toBe(statusItems(row("working"), SCENARIO_PLAN, SCENARIO_PORTS).length);
  });
  it("reads as words with spaces between the parts, the way a status line does", () => {
    expect(text(html())).toBe("Opus 5.5 · high ctx 42% 84k/200k 5h 38% resets in 2h 10m 7d 12% resets in 3d 4h $1.20 1h 05m ~3 changed mcp 3 v2.1.296 skills 14 venv .venv :3000 :5173");
  });
  it("marks every item with its priority, so the stylesheet can hide the least important first", () => {
    const h = html();
    expect(h).toMatch(/data-id="venv"[^>]*data-priority="30"/);
    expect(h).toMatch(/data-id="ctx"[^>]*data-priority="90"/);
    expect(h).toMatch(/data-id="mcp"[^>]*data-priority="60"/);
  });
  it("marks the first item, which never hides, so a separator never leads the line", () => {
    const h = html();
    expect(count(h, /data-first="true"/g)).toBe(1);
    expect(h.indexOf("data-first")).toBeLessThan(h.indexOf('data-id="ctx"'));
    expect(h).toMatch(/data-id="model"[^>]*data-first="true"/);
  });
  it("gives each meter an image name in words and a bar that is decoration", () => {
    const h = html();
    expect(h).toMatch(/role="img" aria-label="Context 42% used, 84k of 200k tokens"/);
    expect(h).toMatch(/aria-label="5-hour limit 38% used, resets in 2h 10m"/);
    expect(count(h, /class="status-strip__bar" aria-hidden="true"/g)).toBe(3);
    expect(h).toContain("inline-size:42%");
    expect(h).toContain("inline-size:38%");
  });
  it("names healthy MCP servers, and shows a named problem with a caution mark and no count", () => {
    expect(html()).toMatch(/data-kind="ok"[^>]*aria-label="MCP servers: 3 connected"/);
    const sick = html("needs-permission");
    expect(sick).toMatch(/data-kind="problem"[^>]*aria-label="MCP problem: discord, 2 of 3 servers connected"/);
    expect(sick).toContain("discord");
    expect(sick).not.toContain('data-kind="ok"');
    expect(count(sick, /status-strip__glyph/g)).toBe(1);
  });
  it("leaves out what is unknown, and is empty, with no group role, when nothing is known", () => {
    const bare = renderToStaticMarkup(<StatusStrip row={{ id: "x", name: "x", project: "p", agent: "claude", status: "idle", phrase: "Idle" }} />);
    expect(bare).toContain('class="status-strip"');
    expect(bare).not.toContain("role=");
    expect(bare).not.toMatch(/status-strip__item/);
    expect(renderToStaticMarkup(<StatusStrip row={null} />)).not.toContain("status-strip__item");
    expect(text(html("done-unseen"))).not.toMatch(/[–]|--|undefined|NaN/);
  });
  it("tones a meter that is nearly full, in its data, for the stylesheet", () => {
    const hot = renderToStaticMarkup(<StatusStrip row={{ ...row("working"), contextPct: 96, usage: undefined }} />);
    expect(hot).toContain('data-tone="error"');
    expect(renderToStaticMarkup(<StatusStrip row={{ ...row("working"), contextPct: 85, usage: undefined }} />)).toContain('data-tone="caution"');
    expect(html()).toContain('data-tone="neutral"');
  });
  it("fits itself to a width in characters when asked: items drop, nothing wraps", () => {
    const wide = html("working", 400);
    const narrow = html("working", 40);
    expect(wide).toContain('data-fit="js"');
    expect(count(narrow, /class="status-strip__item"/g)).toBeLessThan(count(wide, /class="status-strip__item"/g));
    expect(narrow).toContain('data-id="model"');
    expect(narrow).toContain('data-id="ctx"');
    expect(stripWidth(fitItems(statusItems(row("working"), SCENARIO_PLAN, SCENARIO_PORTS), 40))).toBeLessThanOrEqual(40);
  });
  it("puts no literal colour in the markup, and the colours it uses come from tokens", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});
