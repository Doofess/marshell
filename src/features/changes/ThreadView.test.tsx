import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PullNote } from "../../generated/PullNote";
import type { ReviewThread } from "../../generated/ReviewThread";
import type { ThreadComment } from "../../generated/ThreadComment";
import { QUOTE_LINES } from "./comments";
import { NoteView, ThreadView, httpsLink, plainHtml } from "./ThreadView";
import type { SendState } from "./threads";

const html = (s: string) => "<p>" + s + "</p>";
const comment = (over: Partial<ThreadComment> = {}): ThreadComment => ({ id: 1, author: "monalisa", bot: false, body: "Please fix.", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
const thread = (over: Partial<ReviewThread> = {}): ReviewThread => ({
  id: "PRRT_one",
  path: "a.txt",
  side: "new",
  file: false,
  start: 2,
  end: 2,
  original_start: 2,
  original_end: 2,
  resolved: false,
  outdated: false,
  can_resolve: true,
  can_unresolve: true,
  can_reply: true,
  rows: [{ kind: "add", text: "two" }],
  comments: [comment()],
  more_comments: 0,
  minimized: 0,
  ...over,
});
const plain: SendState = { picked: false, note: "", sent: false };
const draw = (t: ReviewThread, o: { placed?: Parameters<typeof ThreadView>[0]["placed"]; state?: SendState; agent?: string | null } = {}) =>
  renderToStaticMarkup(<ThreadView thread={t} placed={o.placed === undefined ? { at: 1, changed: false, start: 2, end: 2 } : o.placed} state={o.state ?? plain} agent={o.agent ?? null} html={html} />);

describe("ThreadView", () => {
  it("heads a thread with GitHub and its first author, and shows each comment's author, once for the first", () => {
    const out = draw(thread({ comments: [comment(), comment({ id: 2, author: "hubot", bot: true, body: "Done." })] }));
    expect(out).toContain("GitHub · monalisa");
    expect(out).toContain("hubot (bot)");
    expect(out.match(/monalisa/g)).toHaveLength(1);
    expect(out).toContain("<p>Please fix.</p>");
    expect(out).toContain("<p>Done.</p>");
    expect(out).toContain('class="markdown changes__thread-body"');
  });
  it("shows each chip that applies, in the order of the label, and none that doesn't", () => {
    const all = draw(thread({ resolved: true, outdated: true, comments: [comment({ hidden_removed: true })] }), { placed: { at: null, changed: true, start: 2, end: 2 }, state: { picked: true, note: "", sent: true }, agent: "Claude Code" });
    const names = ["Resolved", "Outdated", "The line changed", "Hidden text removed", "In this send", "Sent to Claude Code"];
    const at = names.map((n) => all.indexOf(`>${n}</span>`));
    expect(at.every((i) => i > 0)).toBe(true);
    expect([...at].sort((a, b) => a - b)).toEqual(at);
    const none = draw(thread());
    for (const n of names) expect(none).not.toContain(n);
    expect(none).not.toContain("changes__thread-chip");
  });
  it("says Copied for a send with no agent", () => expect(draw(thread(), { state: { picked: false, note: "", sent: true } })).toContain(">Copied</span>"));
  it("names the lines that changed as a range", () => expect(draw(thread({ start: 2, end: 4 }), { placed: { at: 1, changed: true, start: 2, end: 4 } })).toContain("The lines changed"));
  it("shows the place under the head for a thread with no placement or a file comment", () => {
    expect(draw(thread(), { placed: null })).toContain("a.txt:2 (changed since)");
    expect(draw(thread({ file: true, start: null, end: null, rows: [] }), { placed: { at: null, changed: false } })).toContain("On the file");
    expect(draw(thread({ file: true, start: null, end: null, rows: [] }), { placed: null })).toContain("a.txt");
    expect(draw(thread())).not.toContain("a.txt");
  });
  it("draws the code a message quotes when the thread is not placed or its rows changed, and not when the diff shows them", () => {
    const rows = [
      { kind: "context", text: "alpha" },
      { kind: "add", text: "<b>beta</b>" },
    ] as ReviewThread["rows"];
    const unplaced = draw(thread({ rows, start: 2, end: 3 }), { placed: null });
    expect(unplaced).toContain('class="changes__thread-quote"');
    expect(unplaced).toContain("&gt;  alpha");
    expect(unplaced).toContain("&gt; +&lt;b&gt;beta&lt;/b&gt;");
    expect(unplaced).not.toContain("<b>");
    expect(unplaced).not.toContain("…");
    expect(unplaced.indexOf("changes__thread-quote")).toBeGreaterThan(unplaced.indexOf('data-part="place"'));
    expect(unplaced.indexOf("changes__thread-quote")).toBeLessThan(unplaced.indexOf("changes__thread-comment"));
    expect(draw(thread({ rows, start: 2, end: 3 }), { placed: { at: 1, changed: true, start: 2, end: 3 } })).toContain("&gt; +&lt;b&gt;beta&lt;/b&gt;");
    expect(draw(thread({ rows, start: 2, end: 3 }))).not.toContain("changes__thread-quote");
    expect(draw(thread({ file: true, start: null, end: null, rows: [] }), { placed: null })).not.toContain("changes__thread-quote");
  });
  it("marks the rows a quote leaves out with an ellipsis line", () => {
    const rows = Array.from({ length: QUOTE_LINES + 1 }, (_, i) => ({ kind: "add" as const, text: `row${i}` }));
    const out = draw(thread({ rows, start: 2, end: 2 + QUOTE_LINES }), { placed: null });
    expect(out).toContain("&gt; row0");
    expect(out).toContain(`&gt; row${QUOTE_LINES - 1}`);
    expect(out).not.toContain(`row${QUOTE_LINES}`);
    expect(out).toContain("&gt; …");
    expect(draw(thread({ rows: rows.slice(0, QUOTE_LINES), start: 2, end: 1 + QUOTE_LINES }), { placed: null })).not.toContain("…");
  });
  it("shows a date left out when there is none", () => {
    const ms = Date.UTC(2026, 10, 3, 12);
    const day = new Date(ms).toLocaleDateString("en-US", { month: "short", day: "numeric" });
    expect(draw(thread({ comments: [comment({ created_ms: ms })] }))).toContain(day);
    expect(draw(thread())).not.toContain('data-part="date"');
  });
  it("shows the notes under the comments: the cut, the rest, the hidden and the user's own", () => {
    const out = draw(thread({ comments: [comment({ cut: true })], more_comments: 3, minimized: 2 }), { state: { picked: true, note: "only the public one.", sent: false } });
    expect(out).toContain("Cut at 10,000 characters. The rest is on GitHub.");
    expect(out).toContain("And 3 more on GitHub.");
    expect(out).toContain("2 comments are hidden on GitHub.");
    expect(out).toContain("Your note: only the public one.");
    expect(out.indexOf("Your note:")).toBeGreaterThan(out.indexOf("And 3 more"));
    const quiet = draw(thread());
    for (const s of ["Cut at", "more on GitHub", "hidden on GitHub", "Your note"]) expect(quiet).not.toContain(s);
  });
  it("hides all of it from a screen reader, which reads the item's label", () => expect(draw(thread())).toMatch(/^<div[^>]*aria-hidden="true"/));
  it("puts a body in as markup only through html: names, places and notes are text", () => {
    const out = draw(thread({ path: "<i>x</i>.txt", comments: [comment({ author: "<b>m</b>" })] }), { placed: null, state: { picked: false, note: "<u>n</u>", sent: false } });
    for (const tag of ["<b>", "<i>", "<u>"]) expect(out).not.toContain(tag);
    expect(out).toContain("&lt;b&gt;m&lt;/b&gt;");
  });
});

describe("NoteView", () => {
  const note = (over: Partial<PullNote> = {}): PullNote => ({ id: "n1", kind: "review", verdict: "changes_requested", author: "hubot", bot: false, body: "Needs work.", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
  const drawNote = (n: PullNote, state: SendState = plain) => renderToStaticMarkup(<NoteView note={n} state={state} agent={null} html={html} />);
  it("reads Review by … · Changes requested, the marks and the body", () => {
    const out = drawNote(note({ hidden_removed: true }), { picked: true, note: "mine", sent: false });
    expect(out).toContain("Review by hubot · Changes requested");
    expect(out).toContain("Hidden text removed");
    expect(out).toContain("In this send");
    expect(out).toContain("<p>Needs work.</p>");
    expect(out).toContain("Your note: mine");
    expect(out).toMatch(/^<div[^>]*aria-hidden="true"/);
  });
  it("reads Comment by … for a conversation comment, and says when it is cut", () => {
    const out = drawNote(note({ kind: "comment", verdict: null, cut: true }));
    expect(out).toContain("Comment by hubot");
    expect(out).not.toContain("Review by");
    expect(out).toContain("Cut at 10,000 characters.");
  });
});

describe("httpsLink", () => {
  it("is true for an https address only", () => {
    expect(httpsLink("https://example.com/a")).toBe(true);
    expect(httpsLink("http://example.com")).toBe(false);
    expect(httpsLink("javascript:alert(1)")).toBe(false);
    expect(httpsLink("/relative")).toBe(false);
    expect(httpsLink("")).toBe(false);
  });
});

describe("plainHtml", () => {
  it("is the source as escaped text", () => expect(plainHtml("a <b> & c")).toBe("<p>a &lt;b&gt; &amp; c</p>"));
});
