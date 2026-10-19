import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PREVIEW, SETUP } from "../agents/fixtures";
import { OnboardingView, type OnboardingState } from "./LiveOnboarding";

const none = () => {};
const handlers = { onContinue: none, onBack: none, onAllow: none, onSkip: none, onReload: none, onToggleDiff: none, onSwitch: none };
const view = (state: OnboardingState) => renderToStaticMarkup(<OnboardingView state={state} {...handlers} />);

describe("OnboardingView switches", () => {
  it("shows the status line switch off and keeps Allow enabled for a plan without it", () => {
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: PREVIEW, open: false, busy: false, want: { status: true, statusline: false } } });
    expect(h.match(/<input[^>]*role="switch"[^>]*>(?=<span class="switch__track"[^>]*><\/span><span class="switch__text">Keep my)/)?.[0]).not.toContain("checked");
    expect(h).not.toMatch(/class="onboarding__primary"[^>]*disabled/);
  });
  it("disables Allow when status is off, and Skip remains", () => {
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: null, open: false, busy: false, want: { status: false, statusline: true } } });
    expect(h).toMatch(/class="onboarding__primary"[^>]*disabled/);
    expect(h).toContain("Skip: use limited status");
  });
});

describe("OnboardingView", () => {
  it("starts on Detected with the Claude Code row", () => {
    const h = view({ kind: "detected", setup: SETUP });
    expect(h).toContain("Coding agents found");
    expect(h).toContain("Claude Code");
    expect(h).toContain("version 2.1.3");
    expect(h).toContain("C:/h/.claude/settings.json");
    expect(h).toContain(">Continue</button>");
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
    const h = view({ kind: "consent", setup: SETUP, flow: { preview: noChange, open: false, busy: false } });
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
