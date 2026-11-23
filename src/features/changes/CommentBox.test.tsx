import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ChangesReply } from "../../generated/ChangesReply";
import { CommentBox, boxAction, boxLabel, cleanChange } from "./CommentBox";
import { anchorKey, boxAfter } from "./boxModel";
import { anchorOf, place } from "./comments";
import { itemsOf, sectionsOf } from "./model";

const key = (k: string, shiftKey = false) => ({ key: k, shiftKey });
const head = "diff --git a/a.ts b/a.ts\nindex 1..2 100644\n--- a/a.ts\n+++ b/a.ts\n";
const sectionFor = (body: string) => {
  const reply = { kind: "listed", stamp: "s", folder: "r", moved: false, branch: "main", detached: null, base: null, unrelated: false, ahead: 0, uncommitted: true, files: [{ path: "a.ts", old_path: null, status: "modified", note: null }], diff: head + body, truncated: false, more_new: 0, reason: null } as ChangesReply;
  return sectionsOf(reply, {}, [], false);
};
// rows: 0 hunk, 1 one, 2 +two, 3 three
const BODY = "@@ -1,3 +1,3 @@\n one\n+two\n three\n";
// The same lines under five more rows in the same hunk.
const MOVED = "@@ -1,8 +1,8 @@\n p1\n p2\n p3\n p4\n p5\n one\n+two\n three\n";

describe("CommentBox keys", () => {
  it("Enter adds, Shift+Enter breaks the line, Esc keeps typed text", () => {
    expect(boxAction(key("Enter"), "hello")).toBe("add");
    expect(boxAction(key("Enter", true), "hello")).toBe("newline");
    expect(boxAction(key("Escape"), "hello")).toBe("keep");
    expect(boxAction(key("Escape"), "")).toBe("close");
    expect(boxAction(key("Enter"), "")).toBe("close");
    expect(boxAction(key("a"), "x")).toBeNull();
  });
  it("leaves Enter alone while an IME composes", () => {
    expect(boxAction({ key: "Enter", shiftKey: false, isComposing: true }, "x")).toBeNull();
  });
  it("the box drops a pasted escape and form feed", () => {
    expect(cleanChange("a\u001b[201~b\fc")).toBe("a[201~bc");
  });
});

describe("CommentBox markup", () => {
  it("labels the textarea and shows the hint", () => {
    expect(boxLabel(42, 42)).toBe("Comment on line 42");
    expect(boxLabel(10, 14)).toBe("Comment on lines 10 to 14");
    const html = renderToStaticMarkup(<CommentBox label={boxLabel(42, 42)} value="hi" onChange={() => {}} onAction={() => {}} />);
    expect(html).toContain('aria-label="Comment on line 42"');
    expect(html).toContain("Enter adds · Shift+Enter starts a new line · Esc closes");
  });
});

describe("the box's place", () => {
  it("the box keeps its key across a reply that moves lines", () => {
    const before = sectionFor(BODY);
    const after = sectionFor(MOVED);
    const anchor = anchorOf(before[0]!, 2, 2);
    const box = { anchor, id: null };
    const itemsBefore = itemsOf(before, [], new Map());
    const itemsAfter = itemsOf(after, [], new Map());
    const a = boxAfter(itemsBefore, before, box);
    const b = boxAfter(itemsAfter, after, box);
    expect(anchorKey(anchor)).toBe("a.ts\u0000new\u00002\u00002");
    expect(itemsBefore[a]!.kind).toBe("row");
    expect(b).toBe(a + 5);
    expect(place({ ...anchor, id: "", text: "" }, after[0]).changed).toBe(false);
  });
  it("sits under the comment it edits", () => {
    const s = sectionFor(BODY);
    const anchor = anchorOf(s[0]!, 2, 2);
    const c = { ...anchor, id: "c1", text: "t" };
    const items = itemsOf(s, [c], new Map([["c1", place(c, s[0])]]));
    expect(items[boxAfter(items, s, { anchor, id: "c1" })]!.key).toBe("comment:c1");
  });
});
