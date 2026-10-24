import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Conversation } from "../../generated/Conversation";
import type { ToolCard } from "../../generated/ToolCard";
import { FilesChanged } from "./FilesChanged";

const card = (path: string, added?: number, removed?: number, created = false, detail?: string): ToolCard => ({
  name: "Edit",
  summary: `Edited ${path}`,
  detail,
  file: { path, added, removed, created },
  error: false,
});
const conv = (turns: ToolCard[][]): Conversation => ({
  agent: "claude",
  session_id: "a",
  tab_id: "t1",
  title: "t",
  cwd: "C:/dev",
  limited: false,
  marks: { tags: [], pinned: false, archived: false, bookmarked_turns: [] },
  turns: turns.map((cards, index) => ({ index, at_ms: 0, prompt: "p", items: cards.map((c) => ({ kind: "tool" as const, card: c })) })),
});
const html = (c: Conversation) => renderToStaticMarkup(<FilesChanged conversation={c} onGoToTurn={() => {}} />);
const NOTE = "Only edits made with Claude Code's Edit and Write tools are listed. Changes made through shell commands, such as sed or a code generator, aren't.";

describe("FilesChanged", () => {
  const c = conv([[card("a.ts", 2, 1, false, "-x\n+y\n+z")], [], [card("a.ts", 1, 0, false, "+q"), card("new.md", 4, 0, true, "l1\nl2\nl3\nl4"), card("u.ts")]]);

  it("heads the list with the file count, and always shows the note", () => {
    expect(html(c)).toContain("3 files changed");
    expect(html(conv([[card("a.ts", 1, 0)]]))).toContain("1 file changed");
    expect(html(c)).toContain(NOTE.replaceAll("'", "&#x27;"));
  });

  it("shows the totals with a real minus, an en dash for an unknown count, and New file", () => {
    const out = html(c);
    expect(out).toContain("+3 −1");
    expect(out).toContain("+– −–");
    expect(out).toContain("New file");
  });

  it("labels each edit's button by its 1-based turn, and captions the edit", () => {
    const out = html(c);
    expect(out).toContain('aria-label="Go to turn 3"');
    expect(out).toContain('aria-label="Go to turn 1"');
    expect(out).toMatch(/<button[^>]*>Go to turn<\/button>/);
    expect(out).toContain("Turn 3");
  });

  it("says so when nothing was edited", () => {
    const out = html(conv([[]]));
    expect(out).toContain("Claude Code didn&#x27;t edit any files in this conversation.");
    expect(out).toContain("Only edits");
  });

  it("renders a path as text", () => {
    const out = html(conv([[card("<img src=x onerror=alert(1)>.ts", 1, 0)]]));
    expect(out).not.toContain("<img");
    expect(out).toContain("&lt;img src=x onerror=alert(1)&gt;.ts");
  });
});
