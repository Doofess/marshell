import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Conversation } from "../../generated/Conversation";
import type { TurnItem } from "../../generated/TurnItem";
import { ConversationView, NO_ARCHIVE_COPY } from "./ConversationView";
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
  turns: [{ index: 0, at_ms: 0, prompt, items }],
  ...over,
});
const html = (c: Conversation, over: Partial<Parameters<typeof ConversationView>[0]> = {}) =>
  renderToStaticMarkup(<ConversationView conversation={c} onResume={noop} onGoTo={noop} liveTab={null} {...over} />);

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
