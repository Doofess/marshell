import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ChangesReply } from "../../generated/ChangesReply";
import { CommentBox, boxAction, boxLabel, cleanChange, takesFocus } from "./CommentBox";
import { anchorKey, boxAfter } from "./boxModel";
import { anchorOf, place } from "./comments";
import { itemsOf, sectionsOf } from "./model";

const key = (k: string, shiftKey = false) => ({ key: k, shiftKey });
const head = "diff --git a/a.ts b/a.ts\nindex 1..2 100644\n--- a/a.ts\n+++ b/a.ts\n";
const replyFor = (body: string) => ({ kind: "listed", stamp: "s", folder: "r", moved: false, branch: "main", detached: null, base: null, unrelated: false, ahead: 0, uncommitted: true, files: [{ path: "a.ts", old_path: null, status: "modified", note: null }], diff: head + body, truncated: false, more_new: 0, reason: null }) as ChangesReply;
const sectionFor = (body: string) => sectionsOf(replyFor(body), {}, [], false);
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

describe("CommentBox focus", () => {
  // A redraw that mounts the box again (its block rebuilt, a read that failed for a poll) must not pull the keyboard from the terminal.
  it("takes the focus once, for the open the user asked for", () => {
    const flag = { current: true };
    expect(takesFocus(flag)).toBe(true);
    expect(takesFocus(flag)).toBe(false);
    expect(takesFocus(undefined)).toBe(false);
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
  // A file reverted while its box is open keeps the box, and what is typed in it, under the file's "gone" note.
  it("stays under its file's header when the file leaves the diff", () => {
    const anchor = anchorOf(sectionFor(BODY)[0]!, 2, 2);
    const after = sectionsOf({ ...replyFor(BODY), files: [], diff: "" }, {}, [], false, anchor.path);
    const items = itemsOf(after, [], new Map());
    const at = boxAfter(items, after, { anchor, id: null });
    expect(after.map((s) => [s.path, s.note])).toEqual([["a.ts", "gone"]]);
    expect(items[at]!.kind).toBe("note");
    expect(sectionsOf({ ...replyFor(BODY), files: [], diff: "" }, {}, [], false)).toEqual([]);
  });
});
