import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Conversation } from "../../generated/Conversation";
import type { TurnItem } from "../../generated/TurnItem";
import { ConversationView, NO_ARCHIVE_COPY, moreActions, tabKey } from "./ConversationView";
import { MarkdownText } from "./MarkdownText";

vi.mock("../preview/markdown", () => ({ safeMarkdownHtml: vi.fn((md: string, doc: string) => `<p data-sentinel="${doc}">${md.length}</p>`) }));
const { safeMarkdownHtml } = await import("../preview/markdown");

const noop = () => {};
const conv = (items: TurnItem[], over: Partial<Conversation> = {}, prompt = "Fix the rounding"): Conversation => ({
  agent: "claude",
  session_id: "a",
  tab_id: "t1",
  title: "Fix the invoice rounding",
  cwd: "C:\\dev\\billing",
  branch: "main",
  model: "opus",
  version: "2.1.300",
  limited: false,
  marks: { tags: [], pinned: false, archived: false, bookmarked_turns: [] },
  turns: [{ index: 0, at_ms: 0, prompt, items }],
  ...over,
});
const html = (c: Conversation, over: Partial<Parameters<typeof ConversationView>[0]> = {}) =>
  renderToStaticMarkup(<ConversationView
      conversation={c}
      onResume={noop}
      onGoTo={noop}
      liveTab={null}
      onFork={noop}
      onPatch={noop}
      onBookmark={noop}
      onExport={noop}
      onCopyMarkdown={noop}
      onDelete={noop}
      editor={null}
      onEditor={noop}
      {...over}
    />);

describe("ConversationView turns", () => {
  it("makes each turn a section whose h3 is the prompt, as text", () => {
    const out = html(conv([], {}, "<b>x</b> & more"));
    expect(out).toMatch(/<section[^>]*>[\s\S]*<h3[^>]*>&lt;b&gt;x&lt;\/b&gt; &amp; more<\/h3>/);
    expect(out).not.toContain("<b>");
  });

  it("names the conversation, and shows an untitled one with the placeholder", () => {
    expect(html(conv([]))).toMatch(/<h2[^>]*>Fix the invoice rounding<\/h2>/);
    expect(html(conv([], { title: "" }))).toMatch(/<h2[^>]*>Untitled conversation<\/h2>/);
  });

  it("anchors each turn so a hit can scroll to it, and marks the target", () => {
    const c = conv([]);
    c.turns.push({ index: 1, at_ms: 0, prompt: "again", items: [] });
    const out = html(c, { scrollToTurn: 1 });
    const sections = [...out.matchAll(/<section[^>]*>/g)].map((m) => m[0]);
    expect(sections).toHaveLength(2);
    expect(sections[0]).toContain('data-turn="0"');
    expect(sections[0]).not.toContain("data-target");
    expect(sections[1]).toContain('data-turn="1"');
    expect(sections[1]).toContain('data-target="true"');
  });
});

describe("ConversationView items", () => {
  it("draws a tool card as <details> with the summary and the detail in a <pre>, as text", () => {
    const out = html(conv([{ kind: "tool", card: { name: "Bash", summary: "Ran ls", detail: "<script>x</script>", error: false } }]));
    expect(out).toMatch(/<details[^>]*><summary[^>]*>Ran ls<\/summary><pre[^>]*>&lt;script&gt;x&lt;\/script&gt;<\/pre><\/details>/);
    expect(out).not.toContain("data-error");
    expect(out).not.toContain("<script>");
  });

  it("flags a failed card with data-error", () => {
    const out = html(conv([{ kind: "tool", card: { name: "Bash", summary: "Ran false, failed", detail: "exit 1", error: true } }]));
    expect(out).toMatch(/<details[^>]*data-error="true"/);
  });

  it("shows a card with no detail as a plain line, not an empty disclosure", () => {
    const out = html(conv([{ kind: "tool", card: { name: "Read", summary: "Read a.ts", error: false } }]));
    expect(out).toContain("Read a.ts");
    expect(out).not.toContain("<details");
  });

  it("draws a subagent as <details> headed Subagent: …, its prompt and result as text", () => {
    const out = html(conv([{ kind: "subagent", description: "Find callers", prompt: "<i>look</i>", result: "<u>done</u>" }]));
    expect(out).toMatch(/<details[^>]*><summary[^>]*>Subagent: Find callers<\/summary>/);
    expect(out).toContain("&lt;i&gt;look&lt;/i&gt;");
    expect(out).toContain("&lt;u&gt;done&lt;/u&gt;");
    expect(out).not.toContain("<i>");
  });

  it("draws a compaction as a rule and its text", () => {
    const out = html(conv([{ kind: "compacted" }]));
    expect(out).toMatch(/<hr[^>]*\/?>/);
    expect(out).toContain("Conversation compacted");
  });
});

describe("ConversationView notices and buttons", () => {
  it("says the status is limited, with the version, word for word", () => {
    expect(html(conv([], { limited: true, version: "3.0.1" }))).toContain(
      "Status limited: Claude Code 3.0.1 saved this conversation, and Marshell hasn&#x27;t been tested with that version. Some parts may be missing.",
    );
  });
  it("says it without a version, word for word", () => {
    expect(html(conv([], { limited: true, version: undefined }))).toContain(
      "Status limited: this conversation has no Claude Code version, so some parts may be missing.",
    );
  });
  it("shows no notice for a tested version", () => {
    expect(html(conv([]))).not.toContain("Status limited");
  });
  it("offers Resume, or Go to session when a live tab holds it", () => {
    const a = html(conv([]));
    expect(a).toContain(">Resume<");
    expect(a).not.toContain("Go to session");
    const b = html(conv([]), { liveTab: "t9" });
    expect(b).toContain(">Go to session<");
    expect(b).not.toContain(">Resume<");
  });
  it("keeps the no-archive copy exact", () => {
    expect(NO_ARCHIVE_COPY).toBe("Marshell has no copy of this conversation yet.");
  });
});

describe("MarkdownText", () => {
  it("sends assistant text through safeMarkdownHtml when there is a DOM, against a path inside the folder", () => {
    vi.stubGlobal("DOMParser", class {});
    try {
      vi.mocked(safeMarkdownHtml).mockClear();
      const out = renderToStaticMarkup(<MarkdownText markdown="# hi" cwd={"C:\\dev\\billing"} />);
      expect(safeMarkdownHtml).toHaveBeenCalledTimes(1);
      const [md, doc] = vi.mocked(safeMarkdownHtml).mock.calls[0]!;
      expect(md).toBe("# hi");
      // Relative links resolve against the folder of the "document", so the document must sit inside cwd.
      expect(doc.startsWith("C:\\dev\\billing\\")).toBe(true);
      expect(out).toContain('data-sentinel="');
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("uses a forward slash path when the folder does", () => {
    vi.stubGlobal("DOMParser", class {});
    try {
      vi.mocked(safeMarkdownHtml).mockClear();
      renderToStaticMarkup(<MarkdownText markdown="x" cwd="/home/me/app" />);
      expect(vi.mocked(safeMarkdownHtml).mock.calls[0]![1].startsWith("/home/me/app/")).toBe(true);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("without a DOM shows the text as plain pre-wrapped text, never as markup, and never calls the sanitiser", () => {
    expect(typeof DOMParser).toBe("undefined");
    vi.mocked(safeMarkdownHtml).mockClear();
    const out = renderToStaticMarkup(<MarkdownText markdown={"<script>alert(1)</script>\n<img src=x onerror=alert(1)>"} cwd={"C:\\dev"} />);
    expect(out).toContain("&lt;script&gt;alert(1)&lt;/script&gt;");
    expect(out).toContain("&lt;img src=x onerror=alert(1)&gt;");
    expect(out).not.toContain("<script");
    expect(out).not.toContain("<img");
    expect(safeMarkdownHtml).not.toHaveBeenCalled();
  });

  it("renders an assistant text item through it in the conversation", () => {
    vi.stubGlobal("DOMParser", class {});
    try {
      const out = html(conv([{ kind: "text", markdown: "**hello**" }]));
      expect(out).toContain('data-sentinel="C:\\dev\\billing\\');
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe("ConversationView actions", () => {
  it("the header offers Resume, Fork and More actions", () => {
    const out = html(conv([]));
    expect(out).toContain(">Resume<");
    expect(out).toContain(">Fork<");
    expect(out).toMatch(/<button[^>]*aria-haspopup="menu"[^>]*aria-expanded="false"[^>]*>More actions<\/button>/);
  });

  it("while a tab runs the conversation the header offers Go to session, Fork, More actions in that order, and no Resume", () => {
    const out = html(conv([]), { liveTab: "t1" });
    const labels = [...out.matchAll(/<button[^>]*class="conversation__action[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
    expect(labels).toEqual(["Go to session", "Fork", "More actions"]);
  });

  it("the menu lists the actions in order, with Unpin and Unarchive when set", () => {
    const labels = (m: { pinned: boolean; archived: boolean }) => moreActions(m).flatMap((i) => ("separator" in i ? [] : [i.label]));
    expect(labels({ pinned: false, archived: false })).toEqual([
      "Rename…",
      "Tags…",
      "Pin",
      "Archive",
      "Export…",
      "Copy as markdown",
      "Delete Marshell's copy…",
    ]);
    expect(labels({ pinned: true, archived: true })).toEqual([
      "Rename…",
      "Tags…",
      "Unpin",
      "Unarchive",
      "Export…",
      "Copy as markdown",
      "Delete Marshell's copy…",
    ]);
  });

  it("a name with HTML renders as text", () => {
    const hostile = "<img src=x onerror=alert(1)>";
    const out = html(conv([], { title: hostile, marks: { name: hostile, tags: ["<b>x</b>"], pinned: false, archived: false, bookmarked_turns: [] } }), { editor: "rename" });
    expect(out).toContain("&lt;img src=x onerror=alert(1)&gt;");
    expect(out).toContain("&lt;b&gt;x&lt;/b&gt;");
    expect(out).not.toContain("<img");
    expect(out).not.toContain("<b>");
  });

  it("each turn has a Bookmark toggle with aria-pressed", () => {
    const c = conv([], { marks: { tags: [], pinned: false, archived: false, bookmarked_turns: [1] } });
    c.turns.push({ index: 1, at_ms: 0, prompt: "again", items: [] });
    const out = html(c);
    expect(out).toMatch(/<button[^>]*aria-label="Bookmark turn 1"[^>]*aria-pressed="false"[^>]*>Bookmark<\/button>/);
    expect(out).toMatch(/<button[^>]*aria-label="Bookmark turn 2"[^>]*aria-pressed="true"[^>]*>Bookmarked<\/button>/);
  });

  it("the rename box is labelled and hints at Claude Code's title", () => {
    const out = html(conv([]), { editor: "rename" });
    expect(out).toMatch(/<label[^>]*>Conversation name<\/label>/);
    expect(out).toContain("Leave empty to use Claude Code&#x27;s title.");
  });

  it("the delete confirm names the title and says Claude's file is kept", () => {
    const out = html(conv([]), { editor: "delete" });
    expect(out).toContain("Delete Marshell&#x27;s copy of “Fix the invoice rounding”? Claude Code&#x27;s own file is not touched.");
    expect(out).toContain(">Delete<");
    expect(out).toContain(">Cancel<");
  });

  it("the tag error is tied to the box", () => {
    const out = html(conv([]), { editor: "tags", error: "A tag can't contain a comma." });
    const box = out.match(/<input[^>]*aria-describedby="([^"]+)"/);
    expect(box).not.toBeNull();
    const errorId = box![1]!.split(" ").find((id) => out.includes(`id="${id}">A tag can&#x27;t contain a comma.<`));
    expect(errorId).toBeDefined();
    expect(out).toContain("Tags, separated by commas");
  });
});

// These tests have no DOM, so the effect never runs here; the source is checked instead.
describe("ConversationView: scrolling to the target turn", () => {
  it("runs again for another conversation or turn, not each time a refresh replaces the same conversation", () => {
    const src = readFileSync(new URL("./ConversationView.tsx", import.meta.url), "utf8");
    const effect = /scrollIntoView\?\.\([^)]*\)\s*;?\s*\}, \[([^\]]*)\]\);/.exec(src);
    expect(effect).not.toBeNull();
    const deps = effect![1]!.split(",").map((d) => d.trim());
    expect(deps).not.toContain("c");
    expect(deps).toEqual(expect.arrayContaining(["c.agent", "c.session_id", "scrollToTurn"]));
  });
});

describe("ConversationView tabs", () => {
  const two = () => {
    const c = conv([{ kind: "tool", card: { name: "Edit", summary: "Edited a.ts", detail: "", error: false, file: { path: "a.ts", added: 1, removed: 0, created: false, diff: "+x" } } as never }]);
    c.turns.push({ index: 1, at_ms: 2000, prompt: "again", items: [] });
    return c;
  };

  it("draws a tablist with three tabs that each name their panel, and selects one", () => {
    const out = html(two());
    expect(out).toContain('role="tablist"');
    const tabs = [...out.matchAll(/<button[^>]*role="tab"[^>]*>/g)].map((m) => m[0]);
    expect(tabs).toHaveLength(3);
    for (const t of tabs) expect(t).toMatch(/aria-selected="(true|false)"/);
    for (const t of tabs) expect(t).toMatch(/aria-controls="[^"]+"/);
    expect(tabs.filter((t) => t.includes('aria-selected="true"'))).toHaveLength(1);
    expect(tabs.filter((t) => t.includes('tabindex="0"'))).toHaveLength(1);
    expect(out).toContain(">Conversation</button>");
    expect(out).toContain(">Recording</button>");
  });

  it("counts the changed files in the third tab only when there are some", () => {
    expect(html(two())).toContain(">Files changed (1)</button>");
    expect(html(conv([]))).toContain(">Files changed</button>");
  });

  it("shows the panel of the open tab", () => {
    const rec = <p>RECORDING-PANEL</p>;
    const conversation = html(two(), { recording: rec });
    expect(conversation).toContain("again");
    expect(conversation).not.toContain("RECORDING-PANEL");
    const recording = html(two(), { tab: "recording", recording: rec });
    expect(recording).toContain("RECORDING-PANEL");
    expect(recording).not.toContain('data-turn="1"');
    expect(recording).toContain('role="tabpanel"');
    const files = html(two(), { tab: "files" });
    expect(files).toContain("1 file changed");
    expect(files).not.toContain('data-turn="1"');
  });

  it("offers Play from here only on playable turns", () => {
    const out = html(two(), { playableTurns: new Set([1]) });
    expect(out).toContain('aria-label="Play turn 2 from here"');
    expect(out).not.toContain('aria-label="Play turn 1 from here"');
    expect(html(two())).not.toContain("Play from here");
  });
});

describe("tabKey", () => {
  it("moves with the arrows and wraps", () => {
    expect(tabKey("conversation", "ArrowRight")).toBe("recording");
    expect(tabKey("files", "ArrowRight")).toBe("conversation");
    expect(tabKey("conversation", "ArrowLeft")).toBe("files");
    expect(tabKey("recording", "ArrowLeft")).toBe("conversation");
  });
  it("jumps with Home and End, and ignores other keys", () => {
    expect(tabKey("recording", "Home")).toBe("conversation");
    expect(tabKey("recording", "End")).toBe("files");
    expect(tabKey("recording", "a")).toBeNull();
  });
});
