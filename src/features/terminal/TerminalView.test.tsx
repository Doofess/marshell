import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { TerminalView, blockedInput, terminalOptions } from "./TerminalView";

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
