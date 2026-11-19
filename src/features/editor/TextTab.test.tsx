import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Draft } from "../../generated/Draft";
import { start, step, type DocEvent, type DocState } from "./editorDoc";
import { TextTabView, readOnlyLine, type TextTabViewProps, conflictJump, dirtyNews, firstConflict, isSaveKey, keyboardTo, sourceShown, wrapAtStart } from "./TextTab";

// The rendered Markdown needs the DOM, which these tests run without: the stand-in shows the text it was given.
vi.mock("../preview/MarkdownView", () => ({ MarkdownView: ({ text }: { text: string }) => <div data-md="">{text}</div> }));

const N = "a.txt";
const H1 = "1".repeat(64);
const H2 = "2".repeat(64);

/** The state after the events, from a clean open of "one\ntwo". */
function after(...events: DocEvent[]): DocState {
  let s = step(start, { type: "opened", disk: { text: "one\ntwo", hash: H1 }, draft: null }, N).state;
  for (const e of events) s = step(s, e, N).state;
  return s;
}

const noop = () => {};
const props = (doc: DocState, over: Partial<TextTabViewProps> = {}): TextTabViewProps => ({
  entry: { path: "C:\\p\\a.txt", line: null, col: null, view: "source" },
  name: N,
  kind: "text",
  doc,
  ready: true,
  gone: false,
  readOnly: null,
  encoding: "utf-8",
  eol: "lf",
  cursor: { line: 1, column: 1 },
  wrap: true,
  hintId: "hint",
  editor: null,
  onEdit: noop,
  onCursor: noop,
  onMerge: noop,
  onConflict: noop,
  onKeep: noop,
  onTake: noop,
  onUndo: noop,
  onLink: noop,
  onKey: noop,
  ...over,
});
const html = (doc: DocState, over: Partial<TextTabViewProps> = {}) => renderToStaticMarkup(<TextTabView {...props(doc, over)} />);

describe("TextTabView", () => {
  it("a_clean_file_shows_no_bar_and_no_status_line", () => {
    const out = html(after());
    expect(out).not.toContain("changed-bar");
    expect(out).not.toContain("text-tab__status");
    expect(out).toContain('class="editor__host"');
  });

  it("a_conflict_shows_the_bar_with_keep_mine_and_take_disk", () => {
    const draft: Draft = { path: "C:\\p\\a.txt", text: "mine", base_text: "one\ntwo", base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 1 };
    const doc = step(start, { type: "opened", disk: { text: "theirs", hash: H2 }, draft }, N).state;
    const out = html(doc);
    expect(out).toContain('role="group"');
    expect(out).toContain("a.txt changed on disk.");
    expect(out).toContain(">Keep mine<");
    expect(out).toContain(">Take disk<");
    // The bar is not a live region and takes no focus.
    expect(out).not.toContain("aria-live");
    expect(out).not.toContain("autofocus");
  });

  it("the bar offers Merge, Keep mine and Take disk in that order", () => {
    const draft: Draft = { path: "C:\\p\\a.txt", text: "mine", base_text: "one\ntwo", base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 1 };
    const doc = step(start, { type: "opened", disk: { text: "theirs", hash: H2 }, draft }, N).state;
    const out = html(doc);
    const bar = out.slice(out.indexOf("changed-bar"));
    const buttons = [...bar.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
    expect(buttons.slice(0, 3)).toEqual(["Merge", "Keep mine", "Take disk"]);
  });

  it("a read-only tab's bar offers Keep mine and Take disk but no Merge", () => {
    // A read-only tab can't keep the merged text, and the editor there takes no Ctrl+Z (6q-4b decision 7.1).
    const draft: Draft = { path: "C:\\p\\a.txt", text: "mine", base_text: "one\ntwo", base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 1 };
    const doc = step(start, { type: "opened", disk: { text: "theirs", hash: H2 }, draft }, N).state;
    for (const readOnly of ["own", "managed", "attribute"] as const) {
      const out = html(doc, { readOnly });
      const bar = out.slice(out.indexOf("changed-bar"));
      const buttons = [...bar.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
      expect(buttons.slice(0, 2), readOnly).toEqual(["Keep mine", "Take disk"]);
      expect(out, readOnly).not.toContain(">Merge<");
    }
  });

  it("the line under the text shows a merge's result", () => {
    const draft: Draft = { path: "C:\\p\\a.txt", text: "mine", base_text: "one\ntwo", base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 1 };
    let doc = step(start, { type: "opened", disk: { text: "theirs", hash: H2 }, draft }, N).state;
    doc = step(doc, { type: "merge" }, N).state;
    doc = step(doc, { type: "merged", id: 1, text: "<<<<<<< mine\nmine\n=======\ntheirs\n>>>>>>> disk", conflicts: 2, mine: "mine", disk: H2, rendered: false }, N).state;
    const out = html(doc);
    expect(out).toContain("Merged a.txt. 2 conflicts to resolve.");
    expect(out).not.toMatch(/<button[^>]*>Undo<\/button>/);
  });

  it("a_save_shows_saved_with_undo", () => {
    const out = html(after({ type: "edited", text: "x" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }));
    expect(out).toContain("Saved a.txt.");
    expect(out).toMatch(/<button[^>]*>Undo<\/button>/);
  });

  it("no_undo_button_without_a_snapshot", () => {
    const out = html(after({ type: "edited", text: "x" }, { type: "save" }, { type: "saved", hash: H2, snapshot: null }));
    expect(out).toContain("Saved a.txt.");
    expect(out).not.toContain(">Undo<");
  });

  it("the_status_line_sits_under_the_editor_so_a_save_does_not_move_the_text", () => {
    const out = html(after({ type: "edited", text: "x" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }));
    expect(out.indexOf("text-tab__status")).toBeGreaterThan(out.indexOf('data-pane="editor"'));
    expect(out.indexOf("text-tab__status")).toBeLessThan(out.indexOf("editor-footer"));
  });

  it("the_status_line_shows_in_the_rendered_view_while_the_cursors_line_does_not", () => {
    const doc = after({ type: "edited", text: "x" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" });
    const out = html(doc, { kind: "markdown", entry: { path: "C:\p\a.md", line: null, col: null, view: "rendered" } });
    expect(out).toMatch(/<div class="text-tab__status">/);
    expect(out).toMatch(/<div class="text-tab__footer" hidden="">/);
  });

  it("a_tab_says_whether_it_is_unsaved_once_it_is_open_and_then_on_each_change", () => {
    // Nothing before the draft has been looked for: the window still shows what the list of drafts said.
    expect(dirtyNews(null, false, false)).toBeNull();
    // Open and clean: said once, so a dot left by a draft that matched the disk goes.
    expect(dirtyNews(null, true, false)).toBe(false);
    expect(dirtyNews(null, true, true)).toBe(true);
    expect(dirtyNews(false, true, false)).toBeNull();
    expect(dirtyNews(false, true, true)).toBe(true);
    expect(dirtyNews(true, true, false)).toBe(false);
  });

  it("ctrl_s_is_the_save_key_on_any_keyboard_layout", () => {
    const key = (over: Partial<Parameters<typeof isSaveKey>[0]>) => ({ key: "s", code: "KeyS", ctrlKey: true, metaKey: false, altKey: false, shiftKey: false, ...over });
    expect(isSaveKey(key({}), "windows")).toBe(true);
    expect(isSaveKey(key({ key: "S" }), "windows")).toBe(true);
    // A layout with no Latin letters: the key where S is printed.
    expect(isSaveKey(key({ key: "ד" }), "windows")).toBe(true);
    // A Latin layout that puts its letters elsewhere: the key that types s, not the place a QWERTY keyboard has it.
    expect(isSaveKey(key({ key: "o" }), "windows")).toBe(false);
    expect(isSaveKey(key({ code: "Semicolon" }), "windows")).toBe(true);
    expect(isSaveKey(key({ shiftKey: true }), "windows")).toBe(false);
    // AltGr is Ctrl and Alt on Windows.
    expect(isSaveKey(key({ altKey: true }), "windows")).toBe(false);
    expect(isSaveKey(key({ ctrlKey: false }), "windows")).toBe(false);
    expect(isSaveKey(key({ ctrlKey: false, metaKey: true }), "mac")).toBe(true);
    expect(isSaveKey(key({}), "mac")).toBe(false);
  });

  it("a_deleted_file_shows_its_notice", () => {
    const out = html(after({ type: "gone" }));
    expect(out).toContain("a.txt was deleted on disk. Ctrl+S saves it again.");
    // The file's phase says so before the document has caught up.
    expect(html(after(), { gone: true })).toContain("a.txt was deleted on disk. Ctrl+S saves it again.");
    expect(html(after())).not.toContain("was deleted on disk");
  });

  it("each_read_only_reason_shows_its_line", () => {
    expect(html(after(), { readOnly: "own" })).toContain("Marshell doesn&#x27;t save files in its own folder.");
    expect(html(after(), { readOnly: "managed" })).toContain("This file is managed by your organization, so Marshell doesn&#x27;t save it.");
    expect(html(after(), { readOnly: "attribute" })).toContain("a.txt is read-only on disk.");
    expect(readOnlyLine("attribute", "b.md")).toBe("b.md is read-only on disk.");
  });

  it("markdown_renders_the_editors_text_not_the_disks", () => {
    const doc = after({ type: "edited", text: "## Mine heading" });
    expect(doc.base.text).toBe("one\ntwo");
    const out = html(doc, { kind: "markdown", entry: { path: "C:\\p\\a.md", line: null, col: null, view: "rendered" } });
    expect(out).toContain('<div data-md="">## Mine heading</div>');
    // Both views are mounted: the editor stays, hidden, under the rendered one.
    expect(out).toContain('class="editor__host"');
    expect(out).toMatch(/<div class="text-tab__pane"[^>]* hidden=""/);
  });

  it("the_footer_names_utf16_and_crlf", () => {
    const out = html(after(), { encoding: "utf-16le", eol: "crlf", cursor: { line: 3, column: 4 } });
    expect(out).toContain("Ln 3, Col 4");
    expect(out).toContain("UTF-16 LE");
    expect(out).toContain("CRLF");
  });

  it("a_text_over_1_mb_says_the_colours_are_off", () => {
    const big = after({ type: "edited", text: "x".repeat(1024 * 1024 + 1) });
    expect(html(big)).toContain("Syntax colours are off for files over 1 MB.");
    expect(html(after())).not.toContain("Syntax colours");
  });
});

describe("keyboardTo", () => {
  // Undo, Keep mine and Take disk go away when pressed; focus left on a button that is gone falls to the page.
  const spot = () => {
    const calls: string[] = [];
    return { calls, editor: { focus: () => calls.push("editor") }, body: { focus: () => calls.push("body") } };
  };
  it("is_the_editor_while_it_shows", () => {
    const s = spot();
    keyboardTo({ rendered: false, editor: s.editor, body: s.body });
    expect(s.calls).toEqual(["editor"]);
  });
  it("is_the_tabs_body_behind_the_rendered_view_or_with_no_editor_yet", () => {
    const s = spot();
    keyboardTo({ rendered: true, editor: s.editor, body: s.body });
    keyboardTo({ rendered: false, editor: null, body: s.body });
    keyboardTo({ rendered: true, editor: null, body: null });
    expect(s.calls).toEqual(["body", "body"]);
  });
});

describe("conflictJump", () => {
  const spot = () => {
    const jumps: string[] = [];
    const said: string[] = [];
    return { jumps, said, editor: { toConflict: (which: string) => jumps.push(which) }, announce: (t: string) => said.push(t) };
  };
  it("asks_the_editor_while_it_shows", () => {
    const s = spot();
    conflictJump({ rendered: false, editor: s.editor, announce: s.announce }, "next");
    conflictJump({ rendered: false, editor: s.editor, announce: s.announce }, "previous");
    expect(s.jumps).toEqual(["next", "previous"]);
    expect(s.said).toEqual([]);
  });
  it("says_where_conflicts_show_behind_the_rendered_view", () => {
    const s = spot();
    conflictJump({ rendered: true, editor: s.editor, announce: s.announce }, "next");
    expect(s.jumps).toEqual([]);
    expect(s.said).toEqual(["Conflicts show in the Source view."]);
  });
  it("does_nothing_with_no_editor_yet", () => {
    const s = spot();
    conflictJump({ rendered: false, editor: null, announce: s.announce }, "next");
    expect(s.said).toEqual([]);
  });
});

describe("firstConflict", () => {
  // After a merge that left conflicts (6q-4b decision 7.2).
  const spot = () => {
    const calls: string[] = [];
    return { calls, editor: { toConflict: (which: string) => calls.push(which), focus: () => calls.push("focus") }, toSource: () => calls.push("source") };
  };
  it("goes_to_the_first_conflict_while_the_editor_shows", () => {
    const s = spot();
    expect(firstConflict({ rendered: false, editor: s.editor, toSource: s.toSource })).toBe(false);
    expect(s.calls).toEqual(["first"]);
  });
  it("shows_the_source_view_first_and_waits_behind_the_rendered_view", () => {
    const s = spot();
    expect(firstConflict({ rendered: true, editor: s.editor, toSource: s.toSource })).toBe(true);
    expect(s.calls).toEqual(["source"]);
  });
  it("once_the_source_view_shows_takes_the_keyboard_on_from_the_tabs_body_only", () => {
    const s = spot();
    sourceShown({ editor: s.editor, onBody: true });
    expect(s.calls).toEqual(["focus", "first"]);
    s.calls.length = 0;
    sourceShown({ editor: s.editor, onBody: false });
    expect(s.calls).toEqual(["first"]);
    sourceShown({ editor: null, onBody: true });
  });
});

describe("wrapAtStart", () => {
  it("is on for prose and plain text, off for code and markup", () => {
    expect(wrapAtStart("markdown", "C:\p\a.md", "# A")).toBe(true);
    expect(wrapAtStart("text", "C:\p\notes.txt", "one")).toBe(true);
    expect(wrapAtStart("text", "C:\p\notes", undefined)).toBe(true);
    expect(wrapAtStart("code", "C:\p\a.ts", "const a = 1;")).toBe(false);
    expect(wrapAtStart("html", "C:\p\a.html", "<!doctype html>")).toBe(false);
  });
  it("is off for a file whose first line says it is code, though its name does not", () => {
    expect(wrapAtStart("text", "C:\p\script", "#!/usr/bin/env python3")).toBe(false);
  });
});
