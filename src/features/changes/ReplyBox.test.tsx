import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ReplyBox, cleanReply, replyAction } from "./ReplyBox";
import { threadCopy } from "./threadCopy";

const key = (k: string, o: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean; isComposing?: boolean } = {}) => ({ key: k, ctrlKey: false, metaKey: false, shiftKey: false, ...o });

describe("replyAction", () => {
  it("Ctrl+Enter posts a reply that isn't blank", () => {
    expect(replyAction(key("Enter", { ctrlKey: true }), "thanks", false)).toBe("post");
    expect(replyAction(key("Enter", { metaKey: true }), "thanks", false)).toBe("post");
    expect(replyAction(key("Enter", { ctrlKey: true }), "", false)).toBeNull();
    expect(replyAction(key("Enter", { ctrlKey: true }), " \n\t ", false)).toBeNull();
    expect(replyAction(key("Enter", { metaKey: true }), "  ", false)).toBeNull();
  });
  it("Enter alone is the text area's", () => {
    expect(replyAction(key("Enter"), "thanks", false)).toBeNull();
    expect(replyAction(key("Enter", { shiftKey: true }), "thanks", false)).toBeNull();
  });
  it("Esc closes", () => {
    expect(replyAction(key("Escape"), "thanks", false)).toBe("close");
    expect(replyAction(key("Escape"), "", false)).toBe("close");
  });
  it("Esc does nothing while a reply is out", () => {
    expect(replyAction(key("Escape"), "thanks", true)).toBeNull();
    expect(replyAction(key("Escape"), "", true)).toBeNull();
  });
  it("a second post while one is out does nothing", () => {
    expect(replyAction(key("Enter", { ctrlKey: true }), "thanks", true)).toBeNull();
    expect(replyAction(key("Enter", { metaKey: true }), "thanks", true)).toBeNull();
  });
  it("a key during composition does nothing", () => {
    expect(replyAction(key("Enter", { ctrlKey: true, isComposing: true }), "thanks", false)).toBeNull();
    expect(replyAction(key("Escape", { isComposing: true }), "thanks", false)).toBeNull();
  });
  it("leaves every other key to the text area", () => {
    expect(replyAction(key("a"), "x", false)).toBeNull();
    expect(replyAction(key("ArrowDown"), "x", false)).toBeNull();
    expect(replyAction(key("a", { ctrlKey: true }), "x", false)).toBeNull();
  });
});

describe("cleanReply", () => {
  it("keeps line ends and tabs and drops other controls", () => {
    expect(cleanReply("a\r\nb\rc\td\u001b[0me\u0000f\u007fg\u0085h\fi")).toBe("a\nb\nc\td[0mefghi");
  });
  it("keeps what a person writes", () => {
    expect(cleanReply("Fixed in abc123, thanks.\n\n- one\n- two")).toBe("Fixed in abc123, thanks.\n\n- one\n- two");
  });
  it("holds at most 65,536 characters, never cutting a pair", () => {
    const long = "😀".repeat(65_540);
    const out = cleanReply(long);
    expect(Array.from(out)).toHaveLength(65_536);
    expect(cleanReply("a".repeat(65_536))).toHaveLength(65_536);
    expect(cleanReply("a".repeat(65_537))).toHaveLength(65_536);
  });
});

describe("ReplyBox markup", () => {
  const draw = (o: { posting?: boolean; error?: string | null; value?: string } = {}) =>
    renderToStaticMarkup(<ReplyBox label={threadCopy.replyLabel("alice")} value={o.value ?? "thanks"} posting={o.posting ?? false} error={o.error ?? null} onChange={() => {}} onAction={() => {}} focus={{ current: false }} />);
  it("holds the label, the hint and the button", () => {
    const html = draw();
    expect(html).toContain('aria-label="Reply to alice on GitHub"');
    expect(html).toContain("Ctrl+Enter replies on GitHub · Enter starts a new line · Esc closes");
    expect(html).toContain("Reply on GitHub</button>");
    expect(html).toContain('class="changes__box-text"');
    expect(html).toContain('type="button"');
    expect(html).not.toContain('aria-disabled="true"');
    expect(html).not.toContain("disabled=");
  });
  it("reads Replying… on an aria-disabled button while posting, and never disables it", () => {
    const html = draw({ posting: true });
    expect(html).toContain('aria-disabled="true"');
    expect(html).toContain("Replying…</button>");
    expect(html).not.toMatch(/\sdisabled(=|\s|>)/);
  });
  it("shows the error sentence as plain text under the text area, with no live region", () => {
    const html = draw({ error: "GitHub took too long to answer. The reply may be posted: it is not known." });
    expect(html).toContain("GitHub took too long to answer. The reply may be posted: it is not known.");
    expect(html).not.toContain("aria-live");
    expect(html).not.toContain('role="alert"');
    expect(draw()).not.toContain("changes__box-error");
  });
});
