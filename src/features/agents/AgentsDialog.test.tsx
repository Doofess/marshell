import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import { AgentsDialogView, type Panel } from "./AgentsDialog";
import { PREVIEW, SETUP } from "./fixtures";

const none = () => {};
const handlers = { onClose: none, onSetup: none, onHooksOff: none, onBack: none, onAllow: none, onToggleDiff: none, onReload: none, onTurnOff: none };
const view = (setup: AgentSetup | "loading" | "failed", panel: Panel = { kind: "list" }) => renderToStaticMarkup(<AgentsDialogView setup={setup} panel={panel} {...handlers} />);

describe("AgentsDialogView", () => {
  it("is a modal dialog named Agents", () => {
    const h = view(SETUP);
    expect(h).toMatch(/role="dialog"[^>]*aria-modal="true"/);
    expect(h).toMatch(/role="dialog"[^>]*aria-label="Agents"/);
  });
  it("renders the orphaned warning and both buttons", () => {
    const h = view({ ...SETUP, hooks: "orphaned" });
    expect(h).toMatch(/class="agents-dialog__warning"[^>]*>[^<]*the bridge at C:\/m\/bin\/marshell-hook\.exe is missing/);
    expect(h).toContain(">Set up again</button>");
    expect(h).toContain(">Hooks off</button>");
  });
  it("offers only Set up when nothing is installed, and no warning", () => {
    const h = view(SETUP);
    expect(h).toContain(">Set up</button>");
    expect(h).not.toContain("Hooks off</button>");
    expect(h).not.toContain("agents-dialog__warning");
  });
  it("offers only Hooks off when the hooks are on", () => {
    const h = view({ ...SETUP, hooks: "installed" });
    expect(h).toContain("Hooks on in C:/h/.claude/settings.json.");
    expect(h).toContain(">Hooks off</button>");
    expect(h).not.toMatch(/>Set up( again)?<\/button>/);
  });
  it("says when Claude Code is not found, with no buttons but Close", () => {
    const h = view({ ...SETUP, found: false });
    expect(h).toContain("Claude Code was not found on this computer.");
    expect(h).not.toContain("Hooks off</button>");
    expect(h).not.toContain(">Set up</button>");
  });
  it("shows the consent card with the preview diff for Set up", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: PREVIEW, open: true, busy: false } });
    expect(h).toContain("consent");
    expect(h).toContain("&quot;hooks&quot;: {}");
    expect(h).toContain("chain your status line");
  });
  it("shows Reload and no overwrite on a conflict", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: PREVIEW, open: false, busy: false, conflict: true } });
    expect(h).toContain("Review the difference");
    expect(h).toContain(">Reload</button>");
    expect(h).not.toMatch(/overwrite/i);
    expect(h).not.toContain("consent__apply");
  });
  it("shows a refused file's message as the card's error", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: null, open: false, busy: false, error: "hooks is not an object" } });
    expect(h).toMatch(/role="alert"[^>]*>hooks is not an object</);
    expect(h).not.toContain("consent__apply");
  });
  it("shows the removal diff with a Turn hooks off button", () => {
    const h = view({ ...SETUP, hooks: "installed" }, { kind: "off", removal: { preview: PREVIEW, statusline_left: false }, busy: false });
    expect(h).toContain("&quot;hooks&quot;: {}");
    expect(h).toContain(">Turn hooks off</button>");
  });
  it("offers Reload on a removal conflict, never an overwrite", () => {
    const h = view({ ...SETUP, hooks: "installed" }, { kind: "off", removal: { preview: PREVIEW, statusline_left: false }, busy: false, conflict: true });
    expect(h).toContain(">Reload</button>");
    expect(h).not.toContain(">Turn hooks off</button>");
    expect(h).not.toMatch(/overwrite/i);
  });
  it("says why the setup could not be read", () => {
    expect(view("failed")).toContain("Couldn&#x27;t read the setup");
  });
});
