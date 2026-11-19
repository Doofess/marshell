import { afterEach, describe, expect, it, vi } from "vitest";
import type { FileSaved } from "../../../../generated/FileSaved";
import type { SaveResult, UnsaveResult } from "../../../editor/editorApi";
import { escapeLeaves } from "../../Preview";
import { createNotesRunner, notesEscape, onFocusMove } from "./useNotes";

/** A stand-in for an element: `contains` follows the parent chain. */
type Fake = { parent: Fake | null; contains(o: unknown): boolean };
function el(parent: Fake | null = null): Fake {
  const self: Fake = {
    parent,
    contains(o) {
      for (let at = o as Fake | null; at; at = at.parent) if (at === self) return true;
      return false;
    },
  };
  return self;
}
const asEl = (f: Fake | null) => f as unknown as Element | null;

describe("onFocusMove", () => {
  it("the tool is handed to pdf.js only while the keyboard is in the tab", () => {
    const said: boolean[] = [];
    const editor = { setLive: (live: boolean) => void said.push(live) };
    const root = el();
    const page = el(el(root));
    const terminal = el(el());
    onFocusMove(editor, asEl(root), asEl(page), true);
    onFocusMove(editor, asEl(root), asEl(terminal), true);
    onFocusMove(editor, asEl(root), asEl(page), false);
    onFocusMove(editor, asEl(root), null, true);
    onFocusMove(editor, asEl(root), asEl(page), true);
    expect(said).toEqual([true, false, false, false, true]);
  });
});

/** A keydown as the pane's handler sees it, which remembers being default-prevented. */
function key(k: string, mods: Partial<Record<"ctrlKey" | "altKey" | "metaKey" | "shiftKey", boolean>> = {}) {
  const e = {
    key: k,
    ctrlKey: false,
    altKey: false,
    metaKey: false,
    shiftKey: false,
    ...mods,
    defaultPrevented: false,
    preventDefault() {
      e.defaultPrevented = true;
    },
  };
  return e;
}

describe("notesEscape", () => {
  it("escape on the pages goes to the editor first", () => {
    const busy = { escape: () => true };
    const e = key("Escape");
    notesEscape(e, busy);
    expect(escapeLeaves(e, false)).toBe(false);
  });

  it("with nothing selected or typed, Esc goes on to the terminal", () => {
    const idle = { escape: () => false };
    const e = key("Escape");
    notesEscape(e, idle);
    expect(escapeLeaves(e, false)).toBe(true);
    const none = key("Escape");
    notesEscape(none, null);
    expect(escapeLeaves(none, false)).toBe(true);
  });

  it("only a plain Esc asks the editor", () => {
    let asked = 0;
    const editor = { escape: () => (asked++, true) };
    notesEscape(key("Escape", { shiftKey: true }), editor);
    notesEscape(key("Enter"), editor);
    const done = key("Escape");
    done.preventDefault();
    notesEscape(done, editor);
    expect(asked).toBe(0);
  });
});

/** A runner with fakes for the editor and the core, and a log of what reached them in order. */
function rig(o: { draft?: { bytes: ArrayBuffer; baseHash: string | null; editedMs: number } | null; editing?: () => boolean; save?: (path: string, expect: string | null) => SaveResult; failSerializes?: number } = {}) {
  const log: string[] = [];
  let next: ((b: Uint8Array) => void) | null = null;
  let held = false;
  let failing = o.failSerializes ?? 0;
  const editor = {
    save: () => {
      log.push("serialize");
      if (failing > 0 && failing--) return Promise.reject(new TypeError("no layer"));
      if (held) return new Promise<Uint8Array>((r) => (next = r));
      return Promise.resolve(new Uint8Array([7]));
    },
    markSaved: () => void log.push("markSaved"),
    editing: o.editing ?? (() => false),
  };
  const loads: Array<{ bytes: ArrayBuffer; dirty: boolean }> = [];
  const said: string[] = [];
  const opened: string[] = [];
  const api = {
    saveFile: async (path: string, bytes: Uint8Array, expect: string | null): Promise<SaveResult> => {
      log.push(`save ${path} ${expect} [${[...bytes]}]`);
      return o.save?.(path, expect) ?? { kind: "saved", saved: { hash: "h2", snapshot: "s1" } as FileSaved };
    },
    unsaveFile: async (_path: string, snapshot: string): Promise<UnsaveResult> => {
      log.push(`unsave ${snapshot}`);
      return { kind: "saved", saved: { hash: "h1", snapshot: null } as FileSaved };
    },
    putPdfDraft: async (_path: string, bytes: Uint8Array, base: string | null, at: number) => {
      log.push(`put [${[...bytes]}] ${base} ${at}`);
      return "kept" as const;
    },
    dropDraft: async () => void log.push("drop"),
    getPdfDraft: async () => o.draft ?? null,
  };
  const r = createNotesRunner(
    { hash: "h1", bytes: new ArrayBuffer(1) },
    { path: "C:\\w\\a.pdf", name: "a.pdf", editor: () => editor, load: (bytes, dirty) => void loads.push({ bytes, dirty }), announce: (t) => void said.push(t), openCopy: (p) => void opened.push(p), api, now: () => 5 },
  );
  return { r, log, loads, said, opened, hold: () => (held = true), release: (b: Uint8Array) => next?.(b) };
}
const ticks = async (n = 5) => {
  for (let i = 0; i < n; i++) await Promise.resolve();
};

describe("createNotesRunner", () => {
  afterEach(() => vi.useRealTimers());

  it("an unload flushes the draft before the document is destroyed", async () => {
    const t = rig();
    t.hold();
    t.r.dispatch({ type: "edited" });
    // The draft waits for a quiet spell; the tab goes before it ends.
    const done = t.r.unload().then(() => void t.log.push("destroy"));
    expect(t.log).toEqual(["serialize"]);
    await ticks();
    expect(t.log).toEqual(["serialize"]);
    t.release(new Uint8Array([9]));
    await done;
    await ticks();
    expect(t.log[0]).toBe("serialize");
    expect(t.log).toContain("destroy");
    expect(t.log).toContain("put [9] h1 5");
  });

  it("an unload with no draft waiting destroys at once", async () => {
    const t = rig();
    await t.r.unload();
    expect(t.log).toEqual([]);
  });

  it("a draft waits 1500 ms after the last change", async () => {
    vi.useFakeTimers();
    const t = rig();
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1000);
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1499);
    expect(t.log).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    expect(t.log).toEqual(["serialize", "put [7] h1 5"]);
  });

  it("a draft that failed to serialize is taken again when the tab goes", async () => {
    vi.useFakeTimers();
    const t = rig({ failSerializes: 1 });
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    expect(t.log).toEqual(["serialize"]);
    await t.r.unload();
    await ticks();
    expect(t.log).toEqual(["serialize", "serialize", "put [7] h1 5"]);
  });

  it("a draft that failed to serialize is taken again when the window loses focus", async () => {
    vi.useFakeTimers();
    const t = rig({ failSerializes: 1 });
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    t.r.flush();
    await ticks(10);
    expect(t.log).toEqual(["serialize", "serialize", "put [7] h1 5"]);
  });

  it("no draft is taken while a note is being typed: its commit is a change of its own", async () => {
    vi.useFakeTimers();
    let typing = true;
    const t = rig({ editing: () => typing });
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(2000);
    expect(t.log).toEqual([]);
    typing = false;
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    expect(t.log).toEqual(["serialize", "put [7] h1 5"]);
  });

  it("Ctrl+S saves the notes and answers once the file is written", async () => {
    const t = rig();
    t.r.dispatch({ type: "edited" });
    const saved = await t.r.save();
    expect(saved).toBe(true);
    expect(t.log).toEqual(["serialize", "save C:\\w\\a.pdf h1 [7]", "markSaved", "drop"]);
    expect(t.r.state().line).toBe("Saved a.pdf.");
    expect(t.said).toEqual(["Saved a.pdf."]);
  });

  it("Undo takes the save back and the notes are unsaved again", async () => {
    const t = rig();
    t.r.dispatch({ type: "edited" });
    await t.r.save();
    t.r.dispatch({ type: "unsave" });
    await ticks(10);
    expect(t.log).toContain("unsave s1");
    expect(t.r.state()).toMatchObject({ dirty: true, line: "a.pdf is back as it was before the save." });
  });

  it("Save as a copy takes the first free name beside the file and opens it", async () => {
    const t = rig({ save: (path) => (path.endsWith("a (notes).pdf") ? { kind: "conflict", hash: "x" } : { kind: "saved", saved: { hash: "c", snapshot: null } as FileSaved }) });
    t.r.dispatch({ type: "edited" });
    t.r.dispatch({ type: "copy" });
    await ticks(20);
    expect(t.log.filter((l) => l.startsWith("save"))).toEqual(["save C:\\w\\a (notes).pdf null [7]", "save C:\\w\\a (notes 2).pdf null [7]"]);
    expect(t.opened).toEqual(["C:\\w\\a (notes 2).pdf"]);
    expect(t.r.state().line).toBe("Saved the notes in a (notes 2).pdf.");
    // The notes are still unsaved in this file.
    expect(t.r.state().dirty).toBe(true);
  });

  it("discard drops the draft and an unload after it keeps nothing", async () => {
    vi.useFakeTimers();
    const t = rig();
    t.r.dispatch({ type: "edited" });
    t.r.discard();
    await t.r.unload();
    await vi.advanceTimersByTimeAsync(2000);
    expect(t.log).toEqual(["drop"]);
  });

  it("a disk read during a save waits for the answer, and the file as it was before the save is old news", async () => {
    const t = rig();
    t.hold();
    t.r.dispatch({ type: "edited" });
    const saved = t.r.save();
    t.r.disk("h1", new ArrayBuffer(2));
    t.release(new Uint8Array([7]));
    await saved;
    await ticks();
    expect(t.r.state()).toMatchObject({ conflict: null, dirty: false, base: { hash: "h2" } });
    expect(t.loads).toEqual([]);
  });

  it("a change by another program during a save shows once the save is answered", async () => {
    const t = rig({ save: () => ({ kind: "conflict", hash: "h9" }) });
    t.hold();
    t.r.dispatch({ type: "edited" });
    const saved = t.r.save();
    t.r.disk("h9", new ArrayBuffer(3));
    t.release(new Uint8Array([7]));
    expect(await saved).toBe(false);
    await ticks();
    expect(t.r.state().conflict).toMatchObject({ hash: "h9" });
    expect(t.r.state().conflict?.bytes?.byteLength).toBe(3);
  });

  it("a tab opens on its draft: the draft's notes show, unsaved", async () => {
    const draft = new ArrayBuffer(4);
    const t = rig({ draft: { bytes: draft, baseHash: "h1", editedMs: 3 } });
    await t.r.open();
    expect(t.loads).toEqual([{ bytes: draft, dirty: true }]);
    expect(t.r.state()).toMatchObject({ dirty: true, fromDraft: true, conflict: null });
  });

  it("a tab with no draft opens on the file", async () => {
    const t = rig();
    await t.r.open();
    expect(t.loads).toEqual([]);
    expect(t.r.state().dirty).toBe(false);
  });
});
