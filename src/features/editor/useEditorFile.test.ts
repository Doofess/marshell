import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Draft } from "../../generated/Draft";
import { start, type DocEvent, type DocState } from "./editorDoc";
import { DRAFT_MS, createDraftQueue, runEffect, type EffectDeps } from "./useEditorFile";

const N = "a.txt";
const H1 = "1".repeat(64);
const FAILED = "The unsaved edits to a.txt couldn't be kept as a draft.";

type Fakes = { puts: Draft[]; calls: string[]; events: DocEvent[]; said: string[]; sent: Array<{ bytes: Uint8Array; expect: string | null }> };

/** Deps whose I/O is all recorded. `put` decides what a draft put answers. */
function setup(over: { doc?: () => DocState; put?: (d: Draft) => Promise<boolean>; format?: EffectDeps["format"]; save?: EffectDeps["api"]["saveFile"] } = {}) {
  const f: Fakes = { puts: [], calls: [], events: [], said: [], sent: [] };
  let cur: DocState = { ...start, base: { text: "one", hash: H1 }, text: "one", dirty: true };
  const queue = createDraftQueue({
    put: over.put ?? (async (d) => (f.puts.push(d), f.calls.push("put"), true)),
    drop: async () => void f.calls.push("drop"),
    failed: () => f.said.push(FAILED),
  });
  const deps: EffectDeps = {
    path: "C:\\a.txt",
    name: N,
    doc: over.doc ?? (() => cur),
    format: over.format ?? (() => ({ encoding: "utf-8", eol: "lf" })),
    editor: () => null,
    announce: (t) => f.said.push(t),
    refetch: () => f.calls.push("refetch"),
    dispatch: (e) => f.events.push(e),
    drafts: queue,
    now: () => 42,
    api: {
      saveFile:
        over.save ??
        (async (_path, bytes, expect) => {
          f.sent.push({ bytes, expect });
          return { kind: "saved", saved: { size: bytes.length, modified_ms: 1, hash: "h", snapshot: "s" } };
        }),
      unsaveFile: async () => ({ kind: "conflict" }),
    },
  };
  return { f, deps, queue, set: (s: Partial<DocState>) => (cur = { ...cur, ...s }) };
}

describe("runEffect", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("put_draft_waits_400_ms_and_sends_the_newest_text", async () => {
    const { f, deps, set } = setup();
    expect(DRAFT_MS).toBe(400);
    runEffect({ type: "putDraft" }, deps);
    await vi.advanceTimersByTimeAsync(300);
    set({ text: "one two" });
    runEffect({ type: "putDraft" }, deps);
    // The wait starts again with each put.
    await vi.advanceTimersByTimeAsync(399);
    expect(f.puts).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    expect(f.puts).toEqual([{ path: "C:\\a.txt", text: "one two", base_text: "one", base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 42 }]);
    await vi.advanceTimersByTimeAsync(5000);
    expect(f.puts).toHaveLength(1);
  });

  it("drop_draft_cancels_a_waiting_put", async () => {
    const { f, deps } = setup();
    runEffect({ type: "putDraft" }, deps);
    await vi.advanceTimersByTimeAsync(100);
    runEffect({ type: "dropDraft" }, deps);
    await vi.advanceTimersByTimeAsync(5000);
    expect(f.calls).toEqual(["drop"]);
  });

  it("a_drop_waits_for_a_put_on_the_wire_and_no_put_lands_after_it", async () => {
    let finish: (ok: boolean) => void = () => {};
    const { f, deps, queue } = setup({
      put: (d) => {
        f.calls.push("put");
        f.puts.push(d);
        return new Promise<boolean>((resolve) => (finish = resolve));
      },
    });
    runEffect({ type: "putDraft" }, deps);
    queue.flush();
    await vi.advanceTimersByTimeAsync(0);
    expect(f.calls).toEqual(["put"]);
    // A put that is only asked for while another is on the wire, then a drop: the drop goes out after the first put, and the second put never does.
    runEffect({ type: "putDraft" }, deps);
    queue.flush();
    runEffect({ type: "dropDraft" }, deps);
    await vi.advanceTimersByTimeAsync(5000);
    expect(f.calls).toEqual(["put"]);
    finish(true);
    await vi.advanceTimersByTimeAsync(5000);
    expect(f.calls).toEqual(["put", "drop"]);
  });

  it("a_put_after_a_drop_goes_out_after_the_drop", async () => {
    const { f, deps } = setup();
    runEffect({ type: "dropDraft" }, deps);
    runEffect({ type: "putDraft" }, deps);
    await vi.advanceTimersByTimeAsync(1000);
    expect(f.calls).toEqual(["drop", "put"]);
  });

  it("flush_sends_a_waiting_put_at_once", async () => {
    const { f, deps, queue } = setup();
    runEffect({ type: "putDraft" }, deps);
    queue.flush();
    await vi.advanceTimersByTimeAsync(0);
    expect(f.puts).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(5000);
    expect(f.puts).toHaveLength(1);
  });

  it("save_encodes_with_the_files_encoding_and_ending", async () => {
    const { f, deps } = setup({ doc: () => ({ ...start, base: { text: "x", hash: H1 }, text: "a\nb", sent: "a\nb", saving: true }), format: () => ({ encoding: "utf-16le", eol: "crlf" }) });
    await runEffect({ type: "save", expect: H1 }, deps);
    // BOM, then "a", CR, LF, "b", each as a little-endian 16-bit unit.
    expect(Array.from(f.sent[0]!.bytes)).toEqual([0xff, 0xfe, 0x61, 0, 0x0d, 0, 0x0a, 0, 0x62, 0]);
    expect(f.sent[0]!.expect).toBe(H1);
    expect(f.events).toEqual([{ type: "saved", hash: "h", snapshot: "s" }]);
  });

  it("each_save_answer_becomes_its_event", async () => {
    const answer = async (r: Awaited<ReturnType<EffectDeps["api"]["saveFile"]>>) => {
      const { f, deps } = setup({ save: async () => r, doc: () => ({ ...start, text: "x", sent: "x", saving: true }) });
      await runEffect({ type: "save", expect: null }, deps);
      return f.events;
    };
    expect(await answer({ kind: "conflict", hash: "z" })).toEqual([{ type: "saveConflict", hash: "z" }]);
    expect(await answer({ kind: "refused", line: "That path can't be saved." })).toEqual([{ type: "saveFailed", line: "That path can't be saved." }]);
    expect(await answer({ kind: "tooLarge" })).toEqual([{ type: "saveTooLarge" }]);
  });

  it("a_failed_draft_put_announces_once_until_one_succeeds", async () => {
    const answers = [false, false, true, false];
    const { f, deps } = setup({ put: async () => answers.shift()! });
    for (let i = 0; i < 4; i++) {
      runEffect({ type: "putDraft" }, deps);
      await vi.advanceTimersByTimeAsync(DRAFT_MS);
    }
    expect(f.said).toEqual([FAILED, FAILED]);
  });

  it("the_other_effects_go_to_their_owners", async () => {
    const { f, deps } = setup();
    const set: Array<[string, number[]]> = [];
    deps.editor = () => ({ text: () => "", setText: (t, m) => set.push([t, m]), focus() {}, goToLine() {}, toggleWrap() {}, reveal() {} });
    runEffect({ type: "setText", text: "new", marks: [2] }, deps);
    runEffect({ type: "refetch" }, deps);
    runEffect({ type: "announce", text: "hello" }, deps);
    expect(set).toEqual([["new", [2]]]);
    expect(f.calls).toEqual(["refetch"]);
    expect(f.said).toEqual(["hello"]);
  });

  it("an_unsave_that_works_says_unsaved_with_the_new_hash_and_a_conflict_says_refused", async () => {
    const { f, deps } = setup();
    deps.api.unsaveFile = async () => ({ kind: "saved", saved: { size: 1, modified_ms: 1, hash: "back", snapshot: null } });
    await runEffect({ type: "unsave", snapshot: "s" }, deps);
    deps.api.unsaveFile = async () => ({ kind: "conflict" });
    await runEffect({ type: "unsave", snapshot: "s" }, deps);
    deps.api.unsaveFile = async () => ({ kind: "refused", line: "The file is read-only or in use by another program." });
    await runEffect({ type: "unsave", snapshot: "s" }, deps);
    expect(f.events).toEqual([{ type: "unsaved", hash: "back" }, { type: "unsaveRefused" }, { type: "unsaveFailed", line: "The file is read-only or in use by another program." }]);
  });
});
