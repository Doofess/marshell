import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ScriptedTerminal } from "./ScriptedTerminal";

describe("ScriptedTerminal (server markup)", () => {
  it("renders a labelled host element that the client fills with a real xterm", () => {
    const h = renderToStaticMarkup(<ScriptedTerminal />);
    expect(h).toContain('class="scripted-terminal"');
    expect(h).toContain('role="group"');
    expect(h).toContain('aria-label="Terminal preview"');
    expect(h).toContain('data-setting="follow-app"');
  });
  it("carries the setting so the page script can apply it", () => {
    expect(renderToStaticMarkup(<ScriptedTerminal setting="light" />)).toContain('data-setting="light"');
  });
  it("leaves the default session unmarked", () => {
    const a = renderToStaticMarkup(<ScriptedTerminal />);
    expect(a).not.toContain("data-script");
    expect(a).not.toContain("data-cols");
  });
  it("carries another canned session and its size in the markup, so the review page's script mounts that session and not the Claude one", () => {
    const script = "PS C:\\dev\\my-app> git status -s\r\n\u001b[33mM\u001b[0m src/auth.ts\r\nPS C:\\dev\\my-app> ";
    const h = renderToStaticMarkup(<ScriptedTerminal script={script} size={{ cols: 34, rows: 9 }} />);
    const raw = /data-script="([^"]*)"/.exec(h)![1]!;
    const decoded = raw.replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
    expect(JSON.parse(decoded)).toBe(script);
    expect(h).toContain('data-cols="34"');
    expect(h).toContain('data-rows="9"');
  });
  it("uses a role that allows the terminal's own focusable input inside (not img)", () => {
    expect(renderToStaticMarkup(<ScriptedTerminal />)).not.toContain('role="img"');
  });
});
