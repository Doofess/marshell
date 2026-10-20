import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import { AgentsDialogView, type Panel } from "./AgentsDialog";
import { PREVIEW, SETUP } from "./fixtures";
import { nameContainsLabel } from "../../lib/labelInName";

const none = () => {};
const handlers = { onClose: none, onSetup: none, onHooksOff: none, onBack: none, onDone: none, onAllow: none, onToggleDiff: none, onReload: none, onTurnOff: none, onSwitch: none, onLiveStatus: none, onSound: none };
const view = (setup: AgentSetup | "loading" | "failed", panel: Panel = { kind: "list" }, liveStatus: boolean | null = true) => renderToStaticMarkup(<AgentsDialogView setup={setup} panel={panel} liveStatus={liveStatus} {...handlers} />);

describe("AgentsDialogView switches", () => {
  const switchOf = (h: string, label: string) => h.match(new RegExp(`<input[^>]*role="switch"[^>]*>(?=<span class="switch__track"[^>]*></span><span class="switch__text">${label}<)`))?.[0] ?? "";
  it("renders the status line switch unchecked when the person turned it off", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: PREVIEW, open: false, busy: false, want: { status: true, statusline: false, approve: false } } });
    expect(switchOf(h, "Keep my status line working")).not.toContain("checked");
    expect(switchOf(h, "Show status and context")).toContain("checked");
  });
  it("offers no Allow when status is off, and keeps Skip", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: null, open: false, busy: false, want: { status: false, statusline: true, approve: false } } });
    expect(h).not.toContain("consent__apply");
    expect(h).toContain("Skip: use limited status");
  });
});

describe("AgentsDialogView", () => {
  it("offers Set up for approval missing from an installed status setup", () => {
    const h = view({ ...SETUP, hooks: "installed", approve_installed: false });
    expect(h).toContain("Approve from anywhere: off");
    expect(h).toContain(">Set up</button>");
    expect(h).not.toContain(">Set up again</button>");
  });
  it("renders a named keyboard reachable needs-you sound switch with its saved state", () => {
    for (const sound of [true, false]) {
      const h = renderToStaticMarkup(<AgentsDialogView setup={SETUP} panel={{ kind: "list" }} liveStatus={true} sound={sound} {...handlers} onSound={none} />);
      const label = h.match(/<label[^>]*>(?:(?!<\/label>)[\s\S])*?Needs-you sound[\s\S]*?<\/label>/)?.[0] ?? "";
      expect(label).toContain('type="checkbox" role="switch"');
      expect(label.includes('checked=""')).toBe(sound);
      expect(nameContainsLabel(label)).toBe(true);
      expect(label).not.toContain('tabindex="-1"');
    }
  });
  it("leaves the sound switch out until settings are read", () => {
    expect(view(SETUP)).not.toContain("Needs-you sound");
  });
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
    const h = view({ ...SETUP, hooks: "installed", approve_installed: true });
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
    const h = view({ ...SETUP, hooks: "installed" }, { kind: "setup", flow: { preview: noChange, open: false, busy: false } });
    expect(h).not.toContain("consent__apply");
    expect(h).toContain("Already set up. Nothing in C:/h/.claude/settings.json changes.");
    expect(h).toMatch(/consent__skip[^>]*>Done</);
  });
  it("keeps Skip for an empty fresh setup", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: { ...PREVIEW, can_apply: false, files: [] }, open: false, busy: false, want: { status: false, approve: false, statusline: false } } });
    expect(h).toContain("Nothing will change: status is off.");
    expect(h).toMatch(/consent__skip[^>]*>Skip: use limited status</);
    expect(h).not.toContain("Already set up");
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

describe("AgentsDialogView: the Live status switch", () => {
  const sw = (h: string) => h.match(/<input[^>]*role="switch"[^>]*>(?=<span class="switch__track"[^>]*><\/span><span class="switch__text">Live status<)/)?.[0] ?? "";
  it("is on when live status is not switched off for Claude Code", () => {
    expect(sw(view({ ...SETUP, hooks: "installed" }, { kind: "list" }, true))).toContain("checked");
  });
  it("is off when it is", () => {
    const h = sw(view({ ...SETUP, hooks: "installed" }, { kind: "list" }, false));
    expect(h).toContain('role="switch"');
    expect(h).not.toContain("checked");
  });
  it("is left out until the settings have been read", () => {
    expect(view(SETUP, { kind: "list" }, null)).not.toContain("Live status");
  });
});
