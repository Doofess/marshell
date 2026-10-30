import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import { AgentsSectionView, escapeLeavesPanel, SETUP_UNREADABLE, type Panel } from "./AgentsSection";
import { AGY_SETUP, CODEX_SETUP, PREVIEW, SETUP } from "./fixtures";
import { nameContainsLabel } from "../../lib/labelInName";

const none = () => {};
const handlers = { onSetup: none, onHooksOff: none, onBack: none, onDone: none, onAllow: none, onToggleDiff: none, onReload: none, onTurnOff: none, onSwitch: none, onLiveStatus: none, onNotifications: none };
const view = (setup: AgentSetup | "loading" | "failed", panel: Panel = { kind: "list" }, liveStatus: boolean | null = true) => renderToStaticMarkup(<AgentsSectionView setup={setup} panel={panel} liveStatus={liveStatus} {...handlers} />);

describe("AgentsSectionView switches", () => {
  const switchOf = (h: string, label: string) => h.match(new RegExp(`<input[^>]*role="switch"[^>]*>(?=<span class="switch__track"[^>]*></span><span class="switch__text">${label}<)`))?.[0] ?? "";
  it("renders the status line switch unchecked when the person turned it off", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: PREVIEW, open: false, busy: false, want: { status: true, statusline: false, approve: false, skill: false } } });
    expect(switchOf(h, "Keep my status line working")).not.toContain("checked");
    expect(switchOf(h, "Show status and context")).toContain("checked");
  });
  it("offers no Allow when status is off, and keeps Skip", () => {
    const h = view(SETUP, { kind: "setup", flow: { preview: null, open: false, busy: false, want: { status: false, statusline: true, approve: false, skill: false } } });
    expect(h).not.toContain("consent__apply");
    expect(h).toContain("Skip: use limited status");
  });
});

describe("AgentsSectionView skill row", () => {
  const skillHandlers = { install: none, remove: none, apply: none, toggleDiff: none, reload: none, cancel: none };
  const skills = { claude: { flow: null, on: skillHandlers }, codex: { flow: null, on: skillHandlers }, antigravity: { flow: null, on: skillHandlers } };
  const withSkills = (setup: AgentSetup) => renderToStaticMarkup(<AgentsSectionView setup={setup} panel={{ kind: "list" }} liveStatus={true} skills={skills} {...handlers} />);
  it("shows the skill row for a CLI that is found", () => {
    expect(withSkills(SETUP)).toContain('data-agent-skill="claude"');
  });
  it("leaves the skill row out for a CLI that is not found", () => {
    expect(withSkills({ ...SETUP, found: false })).not.toContain("data-agent-skill");
  });
});

describe("AgentsSectionView", () => {
  it("offers Set up for approval missing from an installed status setup", () => {
    const h = view({ ...SETUP, hooks: "installed", approve_installed: false });
    expect(h).toContain("Approve from anywhere: off");
    expect(h).toContain(">Set up</button>");
    expect(h).not.toContain(">Set up again</button>");
  });
  it("points to the Notifications dialog for sounds and banners, with a named button", () => {
    const h = view(SETUP);
    expect(h).toContain("Sounds and banners:");
    expect(h).toMatch(/<button[^>]*>Notifications…<\/button>/);
    expect(h).not.toContain("Needs-you sound");
  });
  it("draws no dialog shell, page title, Close button or live region of its own", () => {
    const h = view(SETUP);
    expect(h).not.toContain('role="dialog"');
    expect(h).not.toContain("<h1");
    expect(h).not.toContain("<h2");
    expect(h).not.toContain(">Close</button>");
    expect(h).not.toContain("aria-live");
    expect(h).not.toContain("role=\"status\"");
  });
  it("lists setup per CLI, then the own agents, then the tools", () => {
    const h = renderToStaticMarkup(<AgentsSectionView setup={SETUP} panel={{ kind: "list" }} liveStatus={true} own={<h3>Your own agents</h3>} tools={<h3>Tools</h3>} {...handlers} />);
    const at = (t: string) => h.indexOf(t);
    expect(at('aria-label="Claude Code"')).toBeGreaterThan(-1);
    expect(at('aria-label="Claude Code"')).toBeLessThan(at("Your own agents"));
    expect(at("Your own agents")).toBeLessThan(at("<h3>Tools</h3>"));
  });
  it("does not say to close a dialog when the setup cannot be read", () => {
    expect(SETUP_UNREADABLE).not.toMatch(/close this/i);
  });
  it("makes each CLI card focusable by script and names it for the start-with focus", () => {
    const h = renderToStaticMarkup(<AgentsSectionView setup={SETUP} codex={CODEX_SETUP} panel={{ kind: "list" }} liveStatus={true} {...handlers} />);
    expect(h).toMatch(/<section[^>]*data-agent-card="codex"[^>]*tabindex="-1"|<section[^>]*tabindex="-1"[^>]*data-agent-card="codex"/);
    expect(h).toMatch(/data-agent-card="claude"/);
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
    const h = view(SETUP, { kind: "setup", flow: { preview: { ...PREVIEW, can_apply: false, files: [] }, open: false, busy: false, want: { status: false, approve: false, statusline: false, skill: false } } });
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

describe("AgentsSectionView: the Live status switch", () => {
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

describe("AgentsSectionView: Codex", () => {
  const withCodex = (codex: AgentSetup | null, panel: Panel = { kind: "list" }, codexLiveStatus: boolean | null = true) =>
    renderToStaticMarkup(<AgentsSectionView setup={SETUP} codex={codex} panel={panel} liveStatus={true} codexLiveStatus={codexLiveStatus} {...handlers} />);
  const codexSection = (h: string) => h.slice(h.indexOf('aria-label="Codex"'));

  it("shows a Codex section with its own consent when Codex is found", () => {
    const list = withCodex(CODEX_SETUP);
    expect(list).toContain('aria-label="Claude Code"');
    expect(codexSection(list)).toContain("C:/bin/codex.exe".slice(0, 0) + "version 0.163.0");
    expect(codexSection(list)).toContain(">Set up</button>");
    const card = withCodex(CODEX_SETUP, { kind: "setup", agent: "codex", flow: { preview: PREVIEW, open: false, busy: false } });
    expect(card).toContain('data-agent="codex"');
    expect(card).toContain("Trust all and continue");
    expect(card).toContain("<code>hooks.json</code> only");
    expect(card).not.toContain("Keep my status line working");
    expect(card).not.toContain("Codex has no status line");
  });
  it("shows no Codex section when Codex is not found or not read", () => {
    expect(withCodex(null)).not.toContain('aria-label="Codex"');
    expect(withCodex({ ...CODEX_SETUP, found: false })).not.toContain('aria-label="Codex"');
  });
  it("Codex's trust line for each trust state, and none for null", () => {
    const line = (trust: AgentSetup["trust"]) => codexSection(withCodex({ ...CODEX_SETUP, hooks: "installed", approve_installed: true, trust }));
    expect(line("trusted")).toContain('<p class="agents-dialog__line">Trusted: Codex runs these hooks.</p>');
    expect(line("untrusted")).toContain(
      '<p class="agents-dialog__warning">Not trusted yet: Codex won&#x27;t run these hooks until you trust them. The next time Codex starts, choose <strong>Trust all and continue</strong>, or review them first in <code>/hooks</code>.</p>',
    );
    expect(line("changed")).toContain(
      '<p class="agents-dialog__warning">Changed since you trusted them: Codex won&#x27;t run the changed hooks until you trust them again in <code>/hooks</code>.</p>',
    );
    const none = line(undefined);
    expect(none).not.toMatch(/Trusted:|Not trusted|Changed since/);
    expect(withCodex(CODEX_SETUP)).not.toMatch(/Trusted:|Not trusted|Changed since/);
    // Claude's section never has a trust line.
    expect(view({ ...SETUP, trust: "trusted" })).not.toContain("Trusted:");
  });
  it("has no status line control, and a Live status switch that is its own", () => {
    const h = codexSection(withCodex(CODEX_SETUP, { kind: "list" }, false));
    expect(h).not.toMatch(/status line/i);
    expect(h).toContain("Live status");
    expect(codexSection(withCodex(CODEX_SETUP, { kind: "list" }, null))).not.toContain("Live status");
  });
  it("names Codex on the turn-off panel", () => {
    const h = withCodex(CODEX_SETUP, { kind: "off", agent: "codex", removal: null, busy: false });
    expect(h).toContain("Turn Codex hooks off");
    expect(h).not.toContain("Turn Claude Code hooks off");
  });
});

describe("AgentsSectionView: Antigravity", () => {
  const withAgy = (agy: AgentSetup | null, panel: Panel = { kind: "list" }, antigravityLiveStatus: boolean | null = true) =>
    renderToStaticMarkup(<AgentsSectionView setup={SETUP} antigravity={agy} panel={panel} liveStatus={true} antigravityLiveStatus={antigravityLiveStatus} {...handlers} />);
  const agySection = (h: string) => h.slice(h.indexOf('aria-label="Antigravity"'));
  const switchLabels = (h: string) => [...h.matchAll(/<span class="switch__text">([^<]*)</g)].map((m) => m[1]);
  const card = (preview = PREVIEW): Panel => ({ kind: "setup", agent: "antigravity", flow: { preview, open: false, busy: false } });

  it("the_agents_dialog_lists_antigravity_with_both_files", () => {
    const h = agySection(withAgy(AGY_SETUP, { kind: "list" }));
    expect(h).toContain("version 1.4.7");
    expect(h).toContain("C:/h/.gemini/config/hooks.json");
    expect(h).toContain("C:/h/.gemini/antigravity-cli/settings.json");
    expect(h).toContain(">Set up</button>");
    expect(h).toContain("Live status");
    expect(withAgy(null)).not.toContain('aria-label="Antigravity"');
    expect(withAgy({ ...AGY_SETUP, found: false })).not.toContain('aria-label="Antigravity"');
    const on = agySection(withAgy({ ...AGY_SETUP, hooks: "installed", approve_installed: true }));
    expect(on).toContain("Hooks on in C:/h/.gemini/config/hooks.json.");
    expect(on).toContain(">Hooks off</button>");
  });
  it("offers status, approve and the status line, and no hand-over, each switch reachable by Tab", () => {
    const h = withAgy(AGY_SETUP, card());
    expect(h).toContain('data-agent="antigravity"');
    expect(switchLabels(h)).toEqual(["Show status and context", "Approve from anywhere", "Keep my status line working", "Install the marshell skill"]);
    expect(h).not.toContain("Let Marshell set the mode");
    expect(h).not.toMatch(/<(input|button)[^>]*tabindex="-1"/);
    expect(h).toContain("Add 5 hook entries in C:/h/.gemini/config/hooks.json (a new file) and chain your status line in C:/h/.gemini/antigravity-cli/settings.json. Add the marshell skill in C:/h/.gemini/config/skills/marshell/SKILL.md.");
  });
  it("the_antigravity_approve_toggle_says_bypass_only", () => {
    expect(withAgy(AGY_SETUP, card())).toContain("Cards for risky commands in Bypass. In other modes Antigravity asks in its terminal.");
    // Claude Code's card says nothing of the kind.
    expect(view(SETUP, { kind: "setup", flow: { preview: PREVIEW, open: false, busy: false } })).not.toContain("Cards for risky commands in Bypass");
  });
  it("antigravity_removal_copy_is_exact", () => {
    expect(withAgy(AGY_SETUP, card())).toContain(
      "Marshell removes the hook entries it added, deletes hooks.json if nothing else is in it, and puts your own status line back from statusline-chain.json; the rest of each file stays as it is",
    );
    const off = withAgy({ ...AGY_SETUP, hooks: "installed" }, { kind: "off", agent: "antigravity", removal: null, busy: false });
    expect(off).toContain("Turn Antigravity hooks off");
  });
  it("a_bridge_path_refusal_shows_its_message", () => {
    const message = "Antigravity can't run Marshell's hook from C:/100%/m/bin/marshell-hook.exe: the path holds a % or a quote.";
    const h = withAgy(AGY_SETUP, { kind: "setup", agent: "antigravity", flow: { preview: null, open: false, busy: false, error: message } });
    expect(h).toMatch(/role="alert"[^>]*>Antigravity can&#x27;t run Marshell&#x27;s hook from C:\/100%\/m\/bin\/marshell-hook\.exe: the path holds a % or a quote\.</);
    expect(h).not.toContain("consent__apply");
  });
});

describe("Esc inside the section", () => {
  const press = (key: string, panel: Panel) => {
    let prevented = false;
    let left = false;
    escapeLeavesPanel({ key, preventDefault: () => (prevented = true) }, panel, () => (left = true));
    return { prevented, left };
  };
  it("on the consent card (with its diff) leaves the card and keeps the page open", () => {
    expect(press("Escape", { kind: "setup", flow: { preview: PREVIEW, open: true, busy: false } })).toEqual({ prevented: true, left: true });
  });
  it("on the Hooks off preview leaves the preview and keeps the page open", () => {
    expect(press("Escape", { kind: "off", removal: { preview: PREVIEW, statusline_left: false }, busy: false })).toEqual({ prevented: true, left: true });
  });
  it("leaves Esc to the page on the list, and ignores other keys", () => {
    expect(press("Escape", { kind: "list" })).toEqual({ prevented: false, left: false });
    expect(press("a", { kind: "off", removal: null, busy: false })).toEqual({ prevented: false, left: false });
  });
});

describe("AgentsSectionView errors", () => {
  const withErrors = (setup: AgentSetup | "loading" | "failed", panel: Panel = { kind: "list" }) =>
    renderToStaticMarkup(<AgentsSectionView setup={setup} panel={panel} liveStatus={true} liveStatusError="Couldn't save the status setting." {...handlers} />);
  it("shows the unreadable setup as plain text: the section announces it through the page, it draws no live region", () => {
    const h = withErrors("failed");
    expect(h).toContain(`>${SETUP_UNREADABLE.replaceAll("'", "&#x27;")}</p>`);
    expect(h).not.toContain('role="alert"');
  });
  it("shows a Live status save error beside the card as plain text", () => {
    const h = withErrors(SETUP);
    expect(h).toContain(">Couldn&#x27;t save the status setting.</p>");
    expect(h).not.toContain('role="alert"');
  });
  it("shows the turn-off panel's error as plain text", () => {
    const h = withErrors(SETUP, { kind: "off", removal: null, busy: false, error: "The file is read-only." });
    expect(h).toContain(">The file is read-only.</p>");
    expect(h).not.toContain('role="alert"');
  });
});
