import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { commandById, shortcutLabel } from "../../lib/keymap";
import { FIXTURES } from "../sidebar/fixtures";
import { Composer, composerState, type ComposerProps } from "./Composer";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;
const render = (id: string, p: Partial<ComposerProps> = {}) => renderToStaticMarkup(<Composer row={row(id)} {...p} />);
const html = (id: string, value = "", os: "windows" | "mac" = "windows", variant: "full" | "compact" = "full") => render(id, { value, os, variant });
const count = (h: string, re: RegExp) => (h.match(re) ?? []).length;
const VARIANTS = ["full", "compact"] as const;

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

describe("Composer: two variants of one command line", () => {
  it("renders the full variant by default, and says which it is", () => {
    expect(html("idle")).toContain('data-variant="full"');
    expect(html("idle", "", "windows", "compact")).toContain('data-variant="compact"');
  });
  it("full has the prompt line and a quiet tools row under it; compact is the prompt line alone", () => {
    const full = html("idle");
    expect(count(full, /class="composer__line"/g)).toBe(1);
    expect(count(full, /class="composer__tools"/g)).toBe(1);
    const compact = html("idle", "", "windows", "compact");
    expect(count(compact, /class="composer__line"/g)).toBe(1);
    expect(compact).not.toContain("composer__tools");
    for (const hidden of ["composer__target", "composer__mode", "composer__hint", "commands", "files"]) expect(compact, hidden).not.toContain(hidden);
  });
  it("both write to the same kind of field: one multi-line field named for the session", () => {
    for (const v of VARIANTS) {
      const h = html("idle", "", "windows", v);
      expect(h, v).toContain("<form");
      expect(h, v).toContain('aria-label="Write to docs-site"');
      expect(h, v).toMatch(/<textarea[^>]*aria-label="Message docs-site"/);
      expect(h, v).toContain('placeholder="Message docs-site…"');
    }
  });
  it("starts the line with a prompt glyph the screen reader skips, like a shell prompt", () => {
    for (const v of VARIANTS) {
      const h = html("idle", "", "windows", v);
      expect(h, v).toMatch(/<svg[^>]*class="composer__prompt"[^>]*aria-hidden="true"/);
      expect(h.indexOf("composer__prompt"), v).toBeLessThan(h.indexOf("<textarea"));
    }
  });
  it("carries the vendor for the accent, and says whether it is the focused prompt", () => {
    for (const v of VARIANTS) expect(html("idle", "", "windows", v), v).toContain('data-agent="codex"');
    expect(render("idle")).toContain('data-focused="true"');
    expect(render("idle", { variant: "compact" })).not.toContain('data-focused="true"');
    expect(render("idle", { variant: "compact", focused: true })).toContain('data-focused="true"');
  });
  it("has a visible Send button with the Enter keycap drawn on it, in both", () => {
    for (const v of VARIANTS) {
      const h = html("idle", "", "windows", v);
      expect(h, v).toMatch(/class="composer__send"[^>]*>[\s\S]*?Send/);
      expect(h, v).toMatch(/class="composer__kbd"[^>]*aria-hidden="true"/);
      expect(h, v).toContain("↵");
    }
  });
  it("has no round icon button: no up-arrow send, no plus", () => {
    for (const v of VARIANTS) {
      const h = html("idle", "", "windows", v);
      expect(h, v).not.toContain("M7 11.5v-9");
      expect(h, v).not.toMatch(/aria-label="(Add|More|\+)[^"]*"/);
    }
  });
});

describe("Composer (full): the line and its quiet tools row", () => {
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
  it("names the focused pane when more than one is on screen, and stays plain for one", () => {
    expect(render("idle", { pane: { index: 1, count: 4 } })).toContain("Pane 2 of 4");
    expect(h).not.toContain("Pane ");
    expect(render("idle", { pane: { index: 0, count: 1 } })).not.toContain("Pane ");
  });
  it("shows the session's mode as plain text, a caution mark for a risky one, and leaves an unknown one out", () => {
    expect(html("working")).toContain("Plan");
    expect(html("working")).not.toMatch(/class="chip"/);
    expect(html("idle")).not.toContain("composer__mode");
    expect(html("bypass")).toMatch(/composer__mode[\s\S]*?aux-glyph/);
  });
});

describe("Composer: states, in both variants", () => {
  for (const v of VARIANTS) {
    describe(v, () => {
      const h = (id: string, value = "") => html(id, value, "windows", v);
      it("empty: Send is present but unavailable, through aria-disabled rather than vanishing", () => {
        expect(h("idle")).toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
      });
      it("typing: Send is available", () => {
        const t = h("idle", "Add retry tests for billing");
        expect(t).not.toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
        expect(t).toContain("Add retry tests for billing");
        expect(t).toContain('data-state="typing"');
      });
      it("working: the button says Queue, and the notice says the message is queued", () => {
        const w = h("working", "also add tests");
        expect(w).toContain("api-server is working");
        expect(w).toContain("queued");
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
      it("an empty pane: nothing to write to, so the field is off and it says why", () => {
        const e = renderToStaticMarkup(<Composer row={null} variant={v} pane={{ index: 2, count: 3 }} />);
        expect(e).toMatch(/<textarea[^>]*disabled/);
        expect(e).toContain("Choose a session for this pane first");
        expect(e).toContain("This pane is empty");
        expect(e).toMatch(/class="composer__send"[^>]*aria-disabled="true"/);
      });
      it("keeps its height in every state: the notice lives in the line (compact) or the tools row (full), never a new row", () => {
        for (const id of ["idle", "working", "needs-permission", "needs-question", "ended"]) {
          const s = h(id);
          expect(s, id).not.toContain("composer__banner");
          expect(count(s, /class="composer__line"/g), id).toBe(1);
          expect(count(s, /class="composer__tools"/g), id).toBe(v === "full" ? 1 : 0);
        }
        expect(h("working")).toContain('class="composer__notice"');
        expect(h("idle")).not.toContain("composer__notice");
      });
    });
  }
});

describe("Composer: hygiene", () => {
  it("is no longer where the status strip lives: that is the status bar", () => {
    for (const v of VARIANTS) expect(html("working", "", "windows", v)).not.toContain("status-strip");
  });
  it("puts no literal colour in the markup", () => {
    for (const v of VARIANTS) for (const id of ["idle", "working", "needs-permission", "ended"]) expect(html(id, "x", "windows", v)).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("has a shortcut to focus it, listed with the other commands", () => {
    expect(shortcutLabel(commandById("focus-prompt").binding!, "windows")).toBe("Ctrl+Shift+M");
    expect(shortcutLabel(commandById("focus-prompt").binding!, "mac")).toBe("⌘M");
    for (const v of VARIANTS) {
      expect(html("idle", "", "windows", v)).toContain("Ctrl+Shift+M");
      expect(html("idle", "", "mac", v)).toContain("⌘M");
    }
  });
});
