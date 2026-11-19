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
function rig(o: { draft?: { bytes: ArrayBuffer; baseHash: string | null; editedMs: number } | null; editing?: () => boolean; save?: (path: string, expect: string | null) => SaveResult | Promise<SaveResult>; failSerializes?: number; readOnly?: () => boolean; now?: () => number } = {}) {
  const log: string[] = [];
  let next: ((b: Uint8Array) => void) | null = null;
  let held = false;
  let present = true;
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
      return (await o.save?.(path, expect)) ?? { kind: "saved", saved: { hash: "h2", snapshot: "s1" } as FileSaved };
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
    { path: "C:\\w\\a.pdf", name: "a.pdf", editor: () => (present ? editor : null), load: (bytes, dirty) => void loads.push({ bytes, dirty }), announce: (t) => void said.push(t), openCopy: (p) => void opened.push(p), api, now: o.now ?? (() => 5), readOnly: o.readOnly },
  );
  // `dropEditor`: the view lets go of the editor right after it calls `unload` (PdfView's cleanup).
  return { r, log, loads, said, opened, hold: () => (held = true), release: (b: Uint8Array) => next?.(b), dropEditor: () => (present = false) };
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

describe("final review 6q-6: Ctrl+S on a read-only file", () => {
  it("says the file is read-only and offers Save as a copy, and writes nothing", async () => {
    const t = rig({ readOnly: () => true });
    t.r.dispatch({ type: "edited" });
    expect(await t.r.save()).toBe(false);
    const line = "a.pdf is read-only, so the notes can't be saved into it.";
    expect(t.r.state()).toMatchObject({ dirty: true, saving: false, refusedSave: true, line });
    expect(t.said).toEqual([line]);
    expect(t.log.filter((l) => l.startsWith("save "))).toEqual([]);
  });
  it("says nothing for a read-only file with no unsaved notes", async () => {
    const t = rig({ readOnly: () => true });
    expect(await t.r.save()).toBe(false);
    expect(t.said).toEqual([]);
    expect(t.r.state().line).toBeNull();
  });
});

describe("final review 6q-6: a change made while a draft is being taken", () => {
  const puts = (log: string[]) => log.filter((l) => l.startsWith("put"));
  it("is in the draft the tab takes when it goes", async () => {
    vi.useFakeTimers();
    const t = rig();
    t.hold();
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    expect(t.log).toEqual(["serialize"]);
    // A second note while the first draft serializes; the tab goes before its own draft is due.
    t.r.dispatch({ type: "edited" });
    const done = t.r.unload().then(() => void t.log.push("destroy"));
    t.release(new Uint8Array([8]));
    await ticks(10);
    t.release(new Uint8Array([9]));
    await done;
    await ticks(10);
    expect(t.log.filter((l) => l === "serialize")).toHaveLength(2);
    expect(t.log.lastIndexOf("serialize")).toBeLessThan(t.log.indexOf("destroy"));
    expect(puts(t.log).at(-1)).toBe("put [9] h1 5");
  });
  it("is in the draft the window's blur sends", async () => {
    vi.useFakeTimers();
    const t = rig();
    t.hold();
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    t.r.dispatch({ type: "edited" });
    t.r.flush();
    t.release(new Uint8Array([8]));
    await ticks(10);
    t.release(new Uint8Array([9]));
    await ticks(10);
    expect(puts(t.log).at(-1)).toBe("put [9] h1 5");
  });
  it("leaves the draft taken before it stamped with the time of the change it holds", async () => {
    vi.useFakeTimers();
    let clock = 5;
    const t = rig({ now: () => clock });
    t.hold();
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    clock = 7;
    t.r.dispatch({ type: "edited" });
    t.release(new Uint8Array([8]));
    await ticks(10);
    expect(puts(t.log)[0]).toBe("put [8] h1 5");
  });
});

describe("final review 6q-6: a tab that goes while a save is in flight", () => {
  const later = <T,>() => {
    let resolve: (v: T) => void = () => {};
    const promise = new Promise<T>((r) => (resolve = r));
    return { promise, resolve };
  };
  it("keeps its notes in a draft when the save is then refused", async () => {
    vi.useFakeTimers();
    const answer = later<SaveResult>();
    const t = rig({ save: () => answer.promise });
    t.r.dispatch({ type: "edited" });
    const saved = t.r.save();
    await ticks();
    let gone = false;
    const done = t.r.unload().then(() => {
      gone = true;
      t.log.push("destroy");
    });
    t.dropEditor();
    await ticks(10);
    // The editor waits for the answer: the notes may not be in the file.
    expect(gone).toBe(false);
    answer.resolve({ kind: "refused", line: "a.pdf is in use." });
    await done;
    await ticks(10);
    expect(await saved).toBe(false);
    expect(t.log.lastIndexOf("serialize")).toBeLessThan(t.log.indexOf("destroy"));
    expect(t.log).toContain("put [7] h1 5");
  });
  it("keeps its notes in a draft when an undone save is answered after it went", async () => {
    vi.useFakeTimers();
    const t = rig();
    t.r.dispatch({ type: "edited" });
    await t.r.save();
    t.log.length = 0;
    t.r.dispatch({ type: "unsave" });
    const done = t.r.unload().then(() => void t.log.push("destroy"));
    t.dropEditor();
    await done;
    await ticks(10);
    expect(t.log.lastIndexOf("serialize")).toBeLessThan(t.log.indexOf("destroy"));
    expect(t.log).toContain("put [7] h1 5");
  });
});

describe("PR review 6q-6: a change made during a save", () => {
  it("made while the save serializes, stays unsaved after it and goes on to a draft", async () => {
    vi.useFakeTimers();
    const t = rig();
    t.hold();
    t.r.dispatch({ type: "edited" });
    const saved = t.r.save();
    // pdf.js took the notes when the save began; this change comes after.
    t.r.dispatch({ type: "edited" });
    t.release(new Uint8Array([7]));
    expect(await saved).toBe(false);
    expect(t.r.state()).toMatchObject({ dirty: true, saving: false, line: "Saved a.pdf." });
    expect(t.log).not.toContain("markSaved");
    expect(t.log).not.toContain("drop");
    await vi.advanceTimersByTimeAsync(1500);
    t.release(new Uint8Array([8]));
    await ticks(10);
    expect(t.log.filter((l) => l.startsWith("put")).at(-1)).toBe("put [8] h2 5");
  });
  it("made while the file is written, is kept in a draft when the tab goes before the answer", async () => {
    vi.useFakeTimers();
    let answer: (r: SaveResult) => void = () => {};
    const t = rig({ save: () => new Promise<SaveResult>((r) => (answer = r)) });
    t.r.dispatch({ type: "edited" });
    void t.r.save();
    await ticks();
    t.r.dispatch({ type: "edited" });
    const done = t.r.unload().then(() => void t.log.push("destroy"));
    t.dropEditor();
    answer({ kind: "saved", saved: { hash: "h2", snapshot: "s1" } as FileSaved });
    await done;
    await ticks(10);
    expect(t.log.lastIndexOf("serialize")).toBeLessThan(t.log.indexOf("destroy"));
    expect(t.log).toContain("put [7] h2 5");
  });
});

describe("PR review 6q-6 round 2: a note still being typed", () => {
  it("is kept in a draft when the tab goes after the timer waited for the typing", async () => {
    vi.useFakeTimers();
    const t = rig({ editing: () => true });
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    // The timer let the typing go on.
    expect(t.log).toEqual([]);
    const done = t.r.unload().then(() => void t.log.push("destroy"));
    t.dropEditor();
    await done;
    await ticks(10);
    expect(t.log.indexOf("serialize")).toBeLessThan(t.log.indexOf("destroy"));
    expect(t.log).toContain("put [7] h1 5");
  });
  it("is still left to the typing when the window loses focus", async () => {
    vi.useFakeTimers();
    const t = rig({ editing: () => true });
    t.r.dispatch({ type: "edited" });
    await vi.advanceTimersByTimeAsync(1500);
    t.r.flush();
    await ticks(10);
    expect(t.log).toEqual([]);
  });
});

describe("PR review 6q-6 round 3: a disk read held during an Overwrite", () => {
  /** A save over h1 meets a file somebody changed to h9; a save that expects h9 (the Overwrite) is answered by `overwrite`. */
  const changed = (overwrite: SaveResult) => (_path: string, expect: string | null): SaveResult => (expect === "h9" ? overwrite : { kind: "conflict", hash: "h9" });
  const barUp = async (overwrite: SaveResult) => {
    const t = rig({ save: changed(overwrite) });
    t.r.dispatch({ type: "edited" });
    expect(await t.r.save()).toBe(false);
    expect(t.r.state().conflict).toEqual({ hash: "h9", bytes: null });
    t.hold();
    t.r.dispatch({ type: "overwrite" });
    // The poll's read of the changed file comes while the overwrite is in flight.
    t.r.disk("h9", new ArrayBuffer(3));
    t.release(new Uint8Array([7]));
    await ticks(10);
    return t;
  };
  it("is old news once the overwrite replaced that file: the tab keeps what it wrote and its Undo", async () => {
    const t = await barUp({ kind: "saved", saved: { hash: "h2", snapshot: "s1" } as FileSaved });
    expect(t.r.state()).toMatchObject({ saving: false, dirty: false, conflict: null, base: { hash: "h2" }, lastSave: { snapshot: "s1", hash: "h2" } });
    expect(t.loads).toEqual([]);
  });
  it("still brings the changed file's bytes when the overwrite was refused, so Reload has them", async () => {
    const t = await barUp({ kind: "refused", line: "a.pdf is in use." });
    expect(t.r.state()).toMatchObject({ saving: false, dirty: true, base: { hash: "h1" } });
    expect(t.r.state().conflict?.hash).toBe("h9");
    expect(t.r.state().conflict?.bytes?.byteLength).toBe(3);
  });
});

describe("PR review 6q-6 round 3: a copy that is refused while a save is in flight", () => {
  const later = <T,>() => {
    let resolve: (v: T) => void = () => {};
    const promise = new Promise<T>((r) => (resolve = r));
    return { promise, resolve };
  };
  it("says its line and leaves the save to its own answer", async () => {
    const copy = later<SaveResult>();
    const file = later<SaveResult>();
    const t = rig({ save: (path) => (path.endsWith("(notes).pdf") ? copy.promise : file.promise) });
    t.r.dispatch({ type: "edited" });
    t.r.dispatch({ type: "copy" });
    await ticks();
    const saved = t.r.save();
    await ticks();
    expect(t.log.filter((l) => l.startsWith("save"))).toEqual(["save C:\\w\\a (notes).pdf null [7]", "save C:\\w\\a.pdf h1 [7]"]);
    copy.resolve({ kind: "refused", line: "The folder is read-only." });
    await ticks(10);
    // The save is still being written: nothing is settled, and the refusal's line is said.
    expect(t.r.state()).toMatchObject({ saving: true, dirty: false, line: "The folder is read-only." });
    file.resolve({ kind: "saved", saved: { hash: "h2", snapshot: "s1" } as FileSaved });
    expect(await saved).toBe(true);
    expect(t.r.state()).toMatchObject({ saving: false, dirty: false, base: { hash: "h2" } });
    expect(t.log).toContain("markSaved");
  });
  it("a copy whose notes couldn't be written leaves the save to its own answer too", async () => {
    const file = later<SaveResult>();
    // The first serialize is the copy's, and fails; the second is the save's.
    const t = rig({ save: () => file.promise, failSerializes: 1 });
    t.r.dispatch({ type: "edited" });
    t.r.dispatch({ type: "copy" });
    const saved = t.r.save();
    await ticks(10);
    expect(t.r.state()).toMatchObject({ saving: true, dirty: false, line: "The notes couldn't be written into a.pdf." });
    file.resolve({ kind: "saved", saved: { hash: "h2", snapshot: "s1" } as FileSaved });
    expect(await saved).toBe(true);
  });
});
