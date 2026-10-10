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
  it("uses a role that allows the terminal's own focusable input inside (not img)", () => {
    expect(renderToStaticMarkup(<ScriptedTerminal />)).not.toContain('role="img"');
  });
});
