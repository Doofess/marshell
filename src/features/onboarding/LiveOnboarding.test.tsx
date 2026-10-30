import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AGY_SETUP, CODEX_SETUP, PREVIEW, SETUP } from "../agents/fixtures";
import { OnboardingView, type OnboardingState } from "./LiveOnboarding";

const none = () => {};
const handlers = { onContinue: none, onBack: none, onAllow: none, onSkip: none, onReload: none, onToggleDiff: none, onSwitch: none };
const view = (state: OnboardingState) => renderToStaticMarkup(<OnboardingView state={state} {...handlers} />);

describe("OnboardingView switches", () => {
  it("shows the status line switch off and keeps Allow enabled for a plan without it", () => {
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: PREVIEW, open: false, busy: false, want: { status: true, statusline: false, approve: true, skill: false } } });
    expect(h.match(/<input[^>]*role="switch"[^>]*>(?=<span class="switch__track"[^>]*><\/span><span class="switch__text">Keep my)/)?.[0]).not.toContain("checked");
    expect(h).not.toMatch(/class="onboarding__primary"[^>]*disabled/);
  });
  it("disables Allow when status is off, and Skip remains", () => {
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: null, open: false, busy: false, want: { status: false, statusline: true, approve: false, skill: false } } });
    expect(h).toMatch(/class="onboarding__primary"[^>]*disabled/);
    expect(h).toContain("Skip: use limited status");
  });
});

describe("OnboardingView", () => {
  it("starts on Detected with the Claude Code row", () => {
    const h = view({ kind: "detected", setup: SETUP, found: null });
    expect(h).toContain("Coding agents found");
    expect(h).toContain("Claude Code");
    expect(h).toContain("version 2.1.3");
    expect(h).toContain("C:/h/.claude/settings.json");
    expect(h).toContain(">Continue</button>");
  });
  it("lists Codex as found, with its program and the note, beside Claude Code", () => {
    const h = view({ kind: "detected", setup: SETUP, found: [{ agent: "claude", found: true }, { agent: "codex", found: true, exe_path: "C:/bin/codex.cmd" }, { agent: "antigravity", found: false }] });
    expect(h).toContain("Codex");
    expect(h).toContain("C:/bin/codex.cmd");
    expect(h).toContain("Live status and setup come in a later version.");
    expect(h).toContain("Not installed");
  });
  it("onboarding_offers_antigravity_when_agy_is_found: Detected lists it from its own setup, then its card names both files", () => {
    const list = view({ kind: "detected", setup: SETUP, found: [{ agent: "antigravity", found: true, exe_path: "C:/bin/agy.exe" }], antigravity: AGY_SETUP });
    expect(list).toContain("Antigravity");
    expect(list).toContain("version 1.4.7");
    expect(list).toContain("C:/h/.gemini/antigravity-cli/settings.json");
    expect(list).not.toContain("Live status and setup come in a later version.");
    const h = view({ kind: "consent", setup: AGY_SETUP, flow: { preview: PREVIEW, open: false, busy: false } });
    expect(h).toContain('data-agent="antigravity"');
    expect(h).toContain("Add 5 hook entries in C:/h/.gemini/config/hooks.json (a new file) and chain your status line in C:/h/.gemini/antigravity-cli/settings.json.");
    expect(h).toContain("Cards for risky commands in Bypass. In other modes Antigravity asks in its terminal.");
    expect(h).not.toContain("Let Marshell set the mode");
    expect(h).toContain(">Allow these changes</button>");
  });
  it("renders the consent card with the preview diff", () => {
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: PREVIEW, open: true, busy: false } });
    expect(h).toContain("chain your status line in C:/h/.claude/settings.json");
    expect(h).toContain("&quot;hooks&quot;: {}");
    expect(h).toContain("Skip: use limited status");
    expect(h).toContain(">Allow these changes</button>");
  });
  it("consent shows reload on a conflict", () => {
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: PREVIEW, open: false, busy: false, conflict: true } });
    expect(h).toContain("Review the difference");
    expect(h).toContain(">Reload</button>");
    expect(h).not.toMatch(/overwrite/i);
    expect(h).toMatch(/class="onboarding__primary"[^>]*disabled/);
  });
  it("shows a refused file's message as the card's error and does not offer Allow", () => {
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: null, open: false, busy: false, error: "hooks is not an object" } });
    expect(h).toMatch(/role="alert"[^>]*>hooks is not an object</);
    expect(h).toMatch(/class="onboarding__primary"[^>]*disabled/);
  });
  it("disables the frame's Allow for a preview that cannot apply", () => {
    const noChange = { ...PREVIEW, can_apply: false, files: PREVIEW.files.map((f) => ({ ...f, diff: "", status: { kind: "noChange" as const } })) };
    const h = view({ kind: "consent", setup: { ...SETUP, hooks: "installed" }, flow: { preview: noChange, open: false, busy: false } });
    expect(h).toMatch(/class="onboarding__primary"[^>]*disabled/);
    expect(h).toContain("Already set up.");
    expect(h).not.toContain("consent__apply");
  });
  it("says when the setup could not be read, and lets the person go on", () => {
    const h = view({ kind: "failed" });
    expect(h).toContain("Couldn&#x27;t read the setup");
    expect(h).toContain(">Skip for now</button>");
  });
});

describe("OnboardingView: Codex", () => {
  it("lists Codex from its own setup, with its version and hooks.json, and no later-version note", () => {
    const h = view({ kind: "detected", setup: SETUP, found: [{ agent: "codex", found: true }], codex: CODEX_SETUP });
    const row = h.slice(h.indexOf('data-agent="codex"'));
    expect(row).toContain("version 0.163.0");
    expect(row).toContain("C:/h/.codex/hooks.json");
    expect(row).not.toContain("come in a later version");
  });
  it("shows Codex's own card with its trust and source lines, and no status line switch", () => {
    const h = view({ kind: "consent", setup: CODEX_SETUP, flow: { preview: PREVIEW, open: false, busy: false } });
    expect(h).toContain('data-agent="codex"');
    expect(h).toContain("<strong>Trust all and continue</strong>");
    expect(h).toContain("<code>hooks.json</code> only");
    expect(h).not.toContain("Keep my status line working");
    expect(h).not.toContain("Claude Code");
  });
});
