import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { commandById, shortcutLabel } from "../../lib/keymap";
import { FIXTURES } from "../sidebar/fixtures";
import { Composer, composerState, type ComposerProps } from "./Composer";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;
const render = (id: string, p: Partial<ComposerProps> = {}) => renderToStaticMarkup(<Composer row={row(id)} {...p} />);
const html = (id: string, value = "", os: "windows" | "mac" = "windows") => render(id, { value, os });
const count = (h: string, re: RegExp) => (h.match(re) ?? []).length;

describe("composerState", () => {
  it("is empty or typing for a session that can take a prompt, ignoring blank space", () => {
    for (const id of ["idle", "done-unseen", "done-seen", "error", "stuck", "limited"]) {
      expect(composerState(row(id), ""), id).toBe("empty");
      expect(composerState(row(id), "  \n "), id).toBe("empty");
      expect(composerState(row(id), "fix it"), id).toBe("typing");
    }
  });
  it("queues for a session that is working", () => {
    expect(composerState(row("working"), "")).toBe("working");
    expect(composerState(row("working"), "also add tests")).toBe("working");
  });
  it("is blocked for a session waiting on the user, so a prompt can never be read by the CLI as the answer", () => {
    expect(composerState(row("needs-permission"), "yes")).toBe("waiting");
    expect(composerState(row("needs-question"), "")).toBe("waiting");
  });
  it("is over for an ended session", () => {
    expect(composerState(row("ended"), "hello")).toBe("ended");
  });
});

describe("Composer: one command line", () => {
  it("has the prompt line and a quiet tools row under it", () => {
    const h = html("idle");
    expect(count(h, /class="composer__line"/g)).toBe(1);
    expect(count(h, /class="composer__tools"/g)).toBe(1);
  });
  it("has no variant: the form carries none", () => {
    expect(html("idle")).not.toContain("data-variant");
  });
  it("writes to one multi-line field named for the session", () => {
    const h = html("idle");
    expect(h).toContain("<form");
    expect(h).toContain('aria-label="Write to docs-site"');
    expect(h).toMatch(/<textarea[^>]*aria-label="Message docs-site"/);
    expect(h).toContain('placeholder="Message docs-site…"');
  });
  it("starts the line with a prompt glyph the screen reader skips, like a shell prompt", () => {
    const h = html("idle");
    expect(h).toMatch(/<svg[^>]*class="composer__prompt"[^>]*aria-hidden="true"/);
    expect(h.indexOf("composer__prompt")).toBeLessThan(h.indexOf("<textarea"));
  });
  it("carries the vendor for the accent", () => {
    expect(html("idle")).toContain('data-agent="codex"');
  });
  it("has a visible Send button with the Enter keycap drawn on it", () => {
    const h = html("idle");
    expect(h).toMatch(/class="composer__send"[^>]*>[\s\S]*?Send/);
    expect(h).toMatch(/class="composer__kbd"[^>]*aria-hidden="true"/);
    expect(h).toContain("↵");
  });
  it("has no round icon button: no up-arrow send, no plus", () => {
    const h = html("idle");
    expect(h).not.toContain("M7 11.5v-9");
    expect(h).not.toMatch(/aria-label="(Add|More|\+)[^"]*"/);
  });
});
describe("Composer: the line and its quiet tools row", () => {
  const h = html("idle");
  it("says how to send, in words", () => {
    expect(h).toContain("Enter to send");
    expect(h).toContain("Shift+Enter for a new line");
  });
  it("shows who it sends to, with the vendor mark, in plain text", () => {
    expect(h).toContain("composer__target");
    expect(h).toContain("agent-mark");
    expect(h).toContain("docs-site");
  });
  it("offers slash commands, file mentions and attach as plain words, named by what they show", () => {
    expect(h).toMatch(/class="composer__tool"[^>]*>[\s\S]*?commands<\/span>/);
    expect(h).toContain(">files<");
    expect(h).toContain(">attach<");
    expect(h).not.toContain('aria-label="Slash commands"');
  });
  it("names no pane number: there is one terminal", () => {
    expect(h).not.toContain("Pane ");
  });
  it("shows the session's mode as plain text, a caution mark for a risky one, and leaves an unknown one out", () => {
    expect(html("working")).toContain("Plan");
    expect(html("working")).not.toMatch(/class="chip"/);
    expect(html("idle")).not.toContain("composer__mode");
    expect(html("bypass")).toMatch(/composer__mode[\s\S]*?aux-glyph/);
  });
});

describe("Composer: states", () => {
  const h = (id: string, value = "") => html(id, value);
  it("empty: Send is present but unavailable, through aria-disabled rather than vanishing", () => {
    expect(h("idle")).toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
  });
  it("typing: Send is available", () => {
    const t = h("idle", "Add retry tests for billing");
    expect(t).not.toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
    expect(t).toContain("Add retry tests for billing");
    expect(t).toContain('data-state="typing"');
  });
  it("working: the button says Queue, with no extra notice", () => {
    const w = h("working", "also add tests");
    expect(w).not.toContain("is working");
    expect(w).not.toContain("composer__notice");
    expect(w).toMatch(/class="composer__send"[^>]*>[\s\S]*?Queue/);
  });
  it("waiting: the field is off, the reason is the placeholder and the notice, and Send is unavailable", () => {
    const w = h("needs-permission", "yes");
    expect(w).toMatch(/<textarea[^>]*disabled/);
    expect(w).toContain("billing is waiting for your approval");
    expect(w).toContain('placeholder="Answer the request in the lane, or in the terminal"');
    expect(w).toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
    expect(h("needs-question")).toContain("infra has a question");
  });
  it("ended: the field is off and the way back is a button", () => {
    const e = h("ended");
    expect(e).toMatch(/<textarea[^>]*disabled/);
    expect(e).toContain("scratch has ended");
    expect(e).toContain("Resume");
  });
  it("keeps its height in every state: the notice lives in the tools row, never a new row", () => {
    for (const id of ["idle", "working", "needs-permission", "needs-question", "ended"]) {
      const s = h(id);
      expect(s, id).not.toContain("composer__banner");
      expect(count(s, /class="composer__line"/g), id).toBe(1);
      expect(count(s, /class="composer__tools"/g), id).toBe(1);
    }
    expect(h("ended")).toContain('class="composer__notice"');
    expect(h("idle")).not.toContain("composer__notice");
  });
});

describe("Composer: one live region for the whole window, not one per prompt line", () => {
  it("never carries a live region of its own, in any state (the window announces the focused session once)", () => {
    for (const id of ["idle", "working", "needs-permission", "needs-question", "ended"])
      expect(html(id), id).not.toMatch(/role="(status|alert|log)"|aria-live=/);
  });
});

describe("Composer: hygiene", () => {
  it("is no longer where the status strip lives: that is the status bar", () => {
    expect(html("working")).not.toContain("status-strip");
  });
  it("puts no literal colour in the markup", () => {
    for (const id of ["idle", "working", "needs-permission", "ended"]) expect(html(id, "x")).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("has a shortcut to focus it, listed with the other commands", () => {
    expect(shortcutLabel(commandById("focus-prompt").binding!, "windows")).toBe("Ctrl+Shift+L");
    expect(shortcutLabel(commandById("focus-prompt").binding!, "mac")).toBe("⌘L");
    expect(html("idle", "", "windows")).toContain("Ctrl+Shift+L");
    expect(html("idle", "", "mac")).toContain("⌘L");
  });
});
