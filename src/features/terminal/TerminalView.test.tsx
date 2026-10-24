import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { TerminalView, blockedInput, isTerminalReport, terminalOptions } from "./TerminalView";

const endpoint: Endpoint = { port: 1, token: "t", platform: "windows" };

describe("TerminalView (server markup)", () => {
  it("shows the placeholder until the first output", () => {
    const h = renderToStaticMarkup(<TerminalView endpoint={endpoint} tabId="t1" placeholder="Starting Claude Code in my-app…" />);
    expect(h).toContain('class="terminal-placeholder"');
    expect(h).toContain("Starting Claude Code in my-app…");
    // Not a live region: the window's announcer is the only one.
    expect(h).not.toContain("role=");
  });
  it("shows nothing extra without one", () => {
    const h = renderToStaticMarkup(<TerminalView endpoint={endpoint} tabId="t1" />);
    expect(h).not.toContain("terminal-placeholder");
  });
});

// These tests have no DOM, so the effect itself never runs here; the source is checked instead.
describe("TerminalView: focus on a new terminal", () => {
  it("runs the focus effect again once the terminal exists, after its font has loaded", () => {
    const src = readFileSync(new URL("./TerminalView.tsx", import.meta.url), "utf8");
    const effect = /if \(autoFocus\) termRef\.current\?\.focus\(\);\s*\}, \[([^\]]*)\]\);/.exec(src);
    expect(effect).not.toBeNull();
    expect(effect![1]!.split(",").map((d) => d.trim())).toContain("fontReady");
  });
});

describe("TerminalView: the replay gate", () => {
  const src = readFileSync(new URL("./TerminalView.tsx", import.meta.url), "utf8");
  it("creates the gate before the colour-scheme tracker that uses it", () => {
    const gate = src.indexOf("new ReplayGate()");
    const tracker = src.search(/trackColorSchemeMode\(\s*term,/);
    expect(gate).toBeGreaterThan(-1);
    expect(tracker).toBeGreaterThan(-1);
    expect(gate).toBeLessThan(tracker);
  });
  it("sends the mode-996 answer only when the gate allows it", () => {
    const call = /trackColorSchemeMode\(\s*term,[^;]*;/.exec(src);
    expect(call).not.toBeNull();
    expect(call![0]).toMatch(/gate\.allows\(/);
  });
});

describe("TerminalView: options", () => {
  it("turns on xterm's proposed API, which the exit-code marks need (registerDecoration throws without it)", () => {
    const o = terminalOptions({ fontFamily: "x", fontSize: 13, theme: {}, minimumContrastRatio: 1 });
    expect(o.allowProposedApi).toBe(true);
    expect(o.scrollback).toBe(10_000);
  });
});

describe("blockedInput", () => {
  it("is true for a live core while the session is paused: the keys are not sent, the bar is shown", () => {
    expect(blockedInput("live", true)).toBe(true);
  });
  it("is false for a running session, and false once the core is gone (nothing to say there)", () => {
    expect(blockedInput("live", false)).toBe(false);
    expect(blockedInput("ended", true)).toBe(false);
    expect(blockedInput("failed", true)).toBe(false);
  });
});

describe("isTerminalReport", () => {
  it("is true for focus reports and mouse reports, which the terminal sends by itself", () => {
    expect(isTerminalReport("\x1b[I")).toBe(true);
    expect(isTerminalReport("\x1b[O")).toBe(true);
    expect(isTerminalReport("\x1b[<0;12;5M")).toBe(true);
    expect(isTerminalReport("\x1b[<0;12;5m")).toBe(true);
    expect(isTerminalReport("\x1b[M !!")).toBe(true);
  });
  it("is false for keystrokes and pastes", () => {
    expect(isTerminalReport("a")).toBe(false);
    expect(isTerminalReport("\r")).toBe(false);
    expect(isTerminalReport("hello world")).toBe(false);
    expect(isTerminalReport("\x1b[A")).toBe(false);
  });
});

