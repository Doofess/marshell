import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import { AgentsDialogView, type Panel } from "./AgentsDialog";
import { PREVIEW, SETUP } from "./fixtures";

const none = () => {};
const handlers = { onClose: none, onSetup: none, onHooksOff: none, onBack: none, onDone: none, onAllow: none, onToggleDiff: none, onReload: none, onTurnOff: none, onSwitch: none };
const view = (setup: AgentSetup | "loading" | "failed", panel: Panel = { kind: "list" }) => renderToStaticMarkup(<AgentsDialogView setup={setup} panel={panel} {...handlers} />);

describe("AgentsDialogView switches", () => {
  const switchOf = (h: string, label: string) => h.match(new RegExp(`<input[^>]*role="switch"[^>]*>(?=<span class="switch__track"[^>]*></span><span class="switch__text">${label}<)`))?.[0] ?? "";
  it("renders the status line switch unchecked when the person turned it off", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: PREVIEW, open: false, busy: false, want: { status: true, statusline: false } } });
    expect(switchOf(h, "Keep my status line working")).not.toContain("checked");
    expect(switchOf(h, "Show status and context")).toContain("checked");
  });
  it("offers no Allow when status is off, and keeps Skip", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: null, open: false, busy: false, want: { status: false, statusline: true } } });
    expect(h).not.toContain("consent__apply");
    expect(h).toContain("Skip: use limited status");
  });
});

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
  it("renders no Allow for a preview with nothing to change, and the card's button reads Done", () => {
    const noChange = { ...PREVIEW, can_apply: false, files: PREVIEW.files.map((f) => ({ ...f, diff: "", status: { kind: "noChange" as const } })) };
    const h = view(SETUP, { kind: "setup", flow: { preview: noChange, open: false, busy: false } });
    expect(h).not.toContain("consent__apply");
    expect(h).toContain("Already set up. Nothing in C:/h/.claude/settings.json changes.");
    expect(h).toMatch(/consent__skip[^>]*>Done</);
  });
  it("renders no Allow and the refusal for a refused file", () => {
    const refused = { ...PREVIEW, can_apply: false, files: [{ ...PREVIEW.files[0]!, diff: "", status: { kind: "refused" as const, reason: "readOnly" as const, detail: "it is read-only" } }] };
    const h = view(SETUP, { kind: "setup", flow: { preview: refused, open: false, busy: false } });
    expect(h).not.toContain("consent__apply");
    expect(h).toContain("Marshell will not edit C:/h/.claude/settings.json: it is read-only");
  });
  it("shows the core's reason for the removal, not the install wording", () => {
    const h = view({ ...SETUP, hooks: "installed" }, { kind: "off", removal: { preview: { ...PREVIEW, reason: "Take Marshell's hooks out. Your status line was left alone." }, statusline_left: false }, busy: false });
    expect(h).toContain("Take Marshell&#x27;s hooks out. Your status line was left alone.");
    expect(h).not.toContain("puts your own status line back");
  });
  it("says why a removal cannot apply, and disables Turn hooks off", () => {
    const refused = { ...PREVIEW, can_apply: false, reason: "r", files: [{ ...PREVIEW.files[0]!, diff: "", status: { kind: "refused" as const, reason: "readOnly" as const, detail: "it is read-only" } }] };
    const h = view({ ...SETUP, hooks: "installed" }, { kind: "off", removal: { preview: refused, statusline_left: false }, busy: false });
    expect(h).toContain("Marshell will not edit C:/h/.claude/settings.json: it is read-only");
    expect(h).toMatch(/<button[^>]*disabled[^>]*>Turn hooks off</);
    const none = { ...PREVIEW, can_apply: false, reason: "Marshell's hooks are not in the file.", files: [] };
    const h2 = view({ ...SETUP, hooks: "installed" }, { kind: "off", removal: { preview: none, statusline_left: false }, busy: false });
    expect(h2).toContain("Marshell&#x27;s hooks are not in the file.");
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
