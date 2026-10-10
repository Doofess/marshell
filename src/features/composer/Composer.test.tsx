import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { commandById, shortcutLabel } from "../../lib/keymap";
import { FIXTURES } from "../sidebar/fixtures";
import { Composer, composerState } from "./Composer";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;
const html = (id: string, value = "", os: "windows" | "mac" = "windows") => renderToStaticMarkup(<Composer row={row(id)} value={value} os={os} />);

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

describe("Composer: it is obviously where you write", () => {
  const h = html("idle");
  it("is a labelled form around one multi-line field named for the session", () => {
    expect(h).toContain("<form");
    expect(h).toContain('aria-label="Write to docs-site"');
    expect(h).toMatch(/<textarea[^>]*aria-label="Message docs-site"/);
    expect(h).toContain('placeholder="Message docs-site…"');
  });
  it("says how to send, in words, next to the field", () => {
    expect(h).toContain("Enter to send");
    expect(h).toContain("Shift+Enter for a new line");
  });
  it("shows who it sends to, with the vendor mark, and carries the vendor for the accent", () => {
    expect(h).toContain("composer__target");
    expect(h).toContain('data-agent="codex"');
    expect(h).toContain("agent-mark");
  });
  it("offers slash commands and file mentions, each with a name and the character that opens it", () => {
    expect(h).toContain('aria-label="Slash commands"');
    expect(h).toContain('aria-label="Mention a file"');
    expect(h).toContain('aria-label="Attach an image"');
  });
  it("has a visible Send button with Enter drawn on it", () => {
    expect(h).toMatch(/class="composer__send"[^>]*>[\s\S]*?Send/);
    expect(h).toContain("↵");
  });
});

describe("Composer: states", () => {
  it("empty: Send is present but unavailable, and says why through aria-disabled rather than vanishing", () => {
    expect(html("idle")).toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
  });
  it("typing: Send is available and carries the vendor accent", () => {
    const h = html("idle", "Add retry tests for billing");
    expect(h).not.toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
    expect(h).toContain("Add retry tests for billing");
    expect(h).toContain('data-state="typing"');
  });
  it("working: says the message will be queued, and the button says Queue", () => {
    const h = html("working", "also add tests");
    expect(h).toContain("api-server is working");
    expect(h).toContain("queued");
    expect(h).toMatch(/class="composer__send"[^>]*>[\s\S]*?Queue/);
  });
  it("waiting: the field is off, the reason is named, and Send is unavailable", () => {
    const h = html("needs-permission", "yes");
    expect(h).toMatch(/<textarea[^>]*disabled/);
    expect(h).toContain("billing is waiting for your approval");
    expect(h).toContain('placeholder="Answer the request in the lane, or in the terminal"');
    expect(h).toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
  });
  it("waiting on a question names a question, not an approval", () => {
    expect(html("needs-question")).toContain("infra has a question");
  });
  it("keeps its height in every state: no banner row, the notice lives in the bar", () => {
    for (const id of ["idle", "working", "needs-permission", "needs-question", "ended"]) {
      const h = html(id);
      expect(h, id).not.toContain("composer__banner");
      expect((h.match(/class="composer__bar"/g) ?? []).length, id).toBe(1);
    }
    expect(html("working")).toContain('class="composer__notice"');
    expect(html("idle")).not.toContain("composer__notice");
  });
  it("an empty pane: nothing to write to, so the field is off and it says why", () => {
    const h = renderToStaticMarkup(<Composer row={null} pane={{ index: 2, count: 3 }} />);
    expect(h).toMatch(/<textarea[^>]*disabled/);
    expect(h).toContain("Choose a session for this pane first");
    expect(h).toContain("This pane is empty");
    expect(h).toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
  });
  it("ended: the field is off and the way back is a button", () => {
    const h = html("ended");
    expect(h).toMatch(/<textarea[^>]*disabled/);
    expect(h).toContain("scratch has ended");
    expect(h).toContain("Resume");
  });
  it("shows the session's mode, and leaves an unknown one out", () => {
    expect(html("working")).toContain("Plan");
    expect(html("idle")).not.toContain("composer__mode");
  });
});

describe("Composer: hygiene", () => {
  it("puts no literal colour in the markup", () => {
    for (const id of ["idle", "working", "needs-permission", "ended"]) expect(html(id, "x")).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("has a shortcut to focus it, listed with the other commands", () => {
    expect(shortcutLabel(commandById("focus-prompt").binding!, "windows")).toBe("Ctrl+Shift+M");
    expect(shortcutLabel(commandById("focus-prompt").binding!, "mac")).toBe("⌘M");
    expect(html("idle")).toContain("Ctrl+Shift+M");
    expect(html("idle", "", "mac")).toContain("⌘M");
  });
});
