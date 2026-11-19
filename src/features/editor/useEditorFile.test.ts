import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Draft } from "../../generated/Draft";
import { start, step, type DocEvent, type DocState } from "./editorDoc";
import type { EditorHandle } from "./Editor";
import type { FileState } from "../preview/usePreviewFile";
import { DRAFT_MS, createDraftQueue, createSaver, draftBusy, draftOnceSent, formatOf, opening, opensOn, runEffect, type EffectDeps } from "./useEditorFile";

const N = "a.txt";
const H1 = "1".repeat(64);
const FAILED = "The unsaved edits to a.txt couldn't be kept as a draft.";

type Fakes = { puts: Draft[]; calls: string[]; events: DocEvent[]; said: string[]; sent: Array<{ bytes: Uint8Array; expect: string | null }> };

/** Deps whose I/O is all recorded. `put` decides what a draft put answers. */
function setup(over: { doc?: () => DocState; put?: (d: Draft) => Promise<boolean>; format?: EffectDeps["format"]; save?: EffectDeps["api"]["saveFile"]; reading?: () => boolean; gone?: () => boolean } = {}) {
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
    rendered: () => false,
    firstConflict: () => f.calls.push("firstConflict"),
    announce: (t) => f.said.push(t),
    reading: over.reading ?? (() => true),
    refetch: () => f.calls.push("refetch"),
    dispatch: (e) => f.events.push(e),
    drafts: queue,
    now: () => 42,
    gone: over.gone ?? (() => false),
    api: {
      saveFile:
        over.save ??
        (async (_path, bytes, expect) => {
          f.sent.push({ bytes, expect });
          return { kind: "saved", saved: { size: bytes.length, modified_ms: 1, hash: "h", snapshot: "s" } };
        }),
      unsaveFile: async () => ({ kind: "conflict" }),
      mergeTexts: async () => ({ kind: "refused", line: "Another merge is still running." }),
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
    deps.editor = () => ({ text: () => "", setText: (t, m) => set.push([t, m]), focus() {}, goToLine() {}, toggleWrap() {}, reveal() {}, toConflict() {} });
    runEffect({ type: "setText", text: "new", marks: [2] }, deps);
    runEffect({ type: "refetch" }, deps);
    runEffect({ type: "announce", text: "hello" }, deps);
    expect(set).toEqual([["new", [2]]]);
    expect(f.calls).toEqual(["refetch"]);
    expect(f.said).toEqual(["hello"]);
  });

  it("the_merge_effect_calls_the_api_and_dispatches_its_answer", async () => {
    const { f, deps } = setup();
    const asked: string[][] = [];
    deps.api.mergeTexts = async (base, mine, disk) => (asked.push([base, mine, disk]), { kind: "merged", text: "merged", conflicts: 2 });
    await runEffect({ type: "merge", id: 3, base: "one", mine: "mine", disk: { text: "other", hash: "2".repeat(64) } }, deps);
    expect(asked).toEqual([["one", "mine", "other"]]);
    expect(f.events).toEqual([{ type: "merged", id: 3, text: "merged", conflicts: 2, mine: "mine", disk: "2".repeat(64), rendered: false }]);
    f.events.length = 0;
    // The answer says which view the tab shows when it comes, not when Merge was pressed.
    deps.rendered = () => true;
    await runEffect({ type: "merge", id: 4, base: "one", mine: "mine", disk: { text: "other", hash: "2".repeat(64) } }, deps);
    expect(f.events).toEqual([{ type: "merged", id: 4, text: "merged", conflicts: 2, mine: "mine", disk: "2".repeat(64), rendered: true }]);
    f.events.length = 0;
    deps.api.mergeTexts = async () => ({ kind: "refused", line: "The merge took too long." });
    await runEffect({ type: "merge", id: 3, base: "one", mine: "mine", disk: { text: "other", hash: "2".repeat(64) } }, deps);
    expect(f.events).toEqual([{ type: "mergeFailed", id: 3, line: "The merge took too long." }]);
  });

  it("a_merge_answered_after_the_tab_went_dispatches_nothing", async () => {
    // The tab was closed with "Don't save" while the merge ran: its answer must not write the draft back.
    for (const answer of [{ kind: "merged", text: "merged", conflicts: 0 }, { kind: "refused", line: "The merge took too long." }] as const) {
      let gone = false;
      const { f, deps } = setup({ gone: () => gone });
      deps.api.mergeTexts = async () => ((gone = true), answer);
      await runEffect({ type: "merge", id: 1, base: "one", mine: "mine", disk: { text: "other", hash: "2".repeat(64) } }, deps);
      expect(f.events, answer.kind).toEqual([]);
    }
  });
  it("the_merge_sends_lf_texts", async () => {
    // A file read with CRLF endings, and a draft over an older version of it: the bar is up, and the merge is asked for.
    const file: FileState = { phase: "text", text: "one\r\ntwo\r\nthree", encoding: "utf-8", eol: "crlf", hash: H1, readOnly: null };
    const draft: Draft = { path: "C:\\a.txt", text: "mine\nmore", base_text: "zero\none", base_hash: "9".repeat(64), encoding: "utf-8", eol: "crlf", edited_ms: 1 };
    const found = await opening("C:\\a.txt", () => file, async () => draft);
    const opened = step(start, { type: "opened", disk: found.disk, draft: found.draft }, N);
    const merge = step(opened.state, { type: "merge" }, N);
    const { f, deps } = setup();
    const asked: string[][] = [];
    deps.api.mergeTexts = async (base, mine, disk) => (asked.push([base, mine, disk]), { kind: "refused", line: "x" });
    for (const effect of merge.effects) await runEffect(effect, deps);
    expect(asked).toEqual([["zero\none", "mine\nmore", "one\ntwo\nthree"]]);
    expect(asked.flat().join("")).not.toContain("\r");
    expect(f.events).toEqual([{ type: "mergeFailed", id: 1, line: "x" }]);
  });
  it("showConflict_hands_the_jump_to_the_tab", () => {
    // The tab knows its view: behind the Rendered view it shows the Source view first (6q-4b decision 7.2).
    const { f, deps } = setup();
    const went: string[] = [];
    const handle: EditorHandle = { text: () => "", setText() {}, focus() {}, goToLine() {}, toggleWrap() {}, reveal() {}, toConflict: (which) => void went.push(which) };
    deps.editor = () => handle;
    runEffect({ type: "showConflict" }, deps);
    expect(f.calls).toEqual(["firstConflict"]);
    expect(went).toEqual([]);
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

describe("discard", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("a_discard_cancels_a_waiting_put_and_the_flush_when_the_tab_goes_writes_nothing", async () => {
    const { f, queue } = setup();
    queue.put(() => ({ path: "C:\\a.txt", text: "one two", base_text: "one", base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 1 }));
    await vi.advanceTimersByTimeAsync(100);
    // What the tab's `discard()` does, and then what its unmount does.
    queue.drop();
    queue.flush();
    await vi.advanceTimersByTimeAsync(5000);
    expect(f.calls).toEqual(["drop"]);
    expect(f.puts).toEqual([]);
  });
});

/** A document run by the real reducer and the real effects, with the saver watching each state, as the hook wires them. */
describe("an announcement for someone reading in the tab", () => {
  it("is_not_spoken_while_the_keyboard_is_elsewhere", () => {
    const { f, deps } = setup({ reading: () => false });
    runEffect({ type: "announce", text: "a.txt changed on disk. 1 line changed.", reading: true }, deps);
    runEffect({ type: "announce", text: "Saved a.txt." }, deps);
    expect(f.said).toEqual(["Saved a.txt."]);
  });
  it("is_spoken_to_someone_whose_keyboard_is_in_the_tab", () => {
    const { f, deps } = setup({ reading: () => true });
    runEffect({ type: "announce", text: "a.txt changed on disk. 1 line changed.", reading: true }, deps);
    expect(f.said).toEqual(["a.txt changed on disk. 1 line changed."]);
  });
});

function saverSetup(from: Partial<DocState>, answer: Awaited<ReturnType<EffectDeps["api"]["saveFile"]>>, readOnly = false) {
  const log = setup();
  let state: DocState = { ...start, base: { text: "one", hash: H1 }, text: "one two", dirty: true, ...from };
  let hold: () => void = () => {};
  const gate = new Promise<void>((r) => (hold = r));
  const deps: EffectDeps = {
    ...log.deps,
    doc: () => state,
    api: {
      ...log.deps.api,
      saveFile: async () => {
        log.f.calls.push("saveFile");
        await gate;
        return answer;
      },
    },
    dispatch: (e) => apply(e),
  };
  const saver = createSaver({ doc: () => state, dispatch: (e) => apply(e), readOnly: () => readOnly });
  function apply(e: DocEvent) {
    const out = step(state, e, N);
    state = out.state;
    saver.observe(state);
    for (const effect of out.effects) void runEffect(effect, deps);
  }
  return { ...log, saver, answer: () => hold(), state: () => state };
}

describe("createSaver", () => {
  const saved = { kind: "saved", saved: { size: 7, modified_ms: 1, hash: "h2", snapshot: "s" } } as const;

  it("save_resolves_true_after_the_save_is_answered", async () => {
    const t = saverSetup({}, saved);
    let result: boolean | undefined;
    void t.saver.save().then((r) => (result = r));
    await Promise.resolve();
    expect(result).toBeUndefined();
    t.answer();
    await vi.waitFor(() => expect(result).toBe(true));
    expect(t.state().dirty).toBe(false);
  });

  it("save_resolves_false_after_a_refusal_and_after_a_conflict", async () => {
    for (const answer of [{ kind: "refused", line: "The file is read-only or in use by another program." }, { kind: "conflict", hash: "other" }, { kind: "tooLarge" }] as const) {
      const t = saverSetup({}, answer as never);
      const result = t.saver.save();
      t.answer();
      expect(await result).toBe(false);
      expect(t.state().dirty).toBe(true);
    }
  });

  it("save_resolves_false_at_once_while_the_bar_is_up", async () => {
    const t = saverSetup({ conflict: { text: "theirs", hash: "other" } }, saved);
    expect(await t.saver.save()).toBe(false);
    expect(t.f.calls).toEqual([]);
  });

  it("save_resolves_true_at_once_with_nothing_to_save", async () => {
    const t = saverSetup({ dirty: false, text: "one" }, saved);
    expect(await t.saver.save()).toBe(true);
    expect(t.f.calls).toEqual([]);
  });

  it("save_resolves_false_for_a_read_only_file_without_asking_the_core", async () => {
    const t = saverSetup({}, saved, true);
    expect(await t.saver.save()).toBe(false);
    expect(t.f.calls).toEqual([]);
  });

  it("a_save_before_the_document_is_open_is_nothing", async () => {
    // The draft is still being looked for: the state is the empty start, and a save of it would ask the core to make an empty file.
    const events: DocEvent[] = [];
    const saver = createSaver({ doc: () => start, dispatch: (e) => events.push(e), readOnly: () => false, open: () => false });
    expect(await saver.save()).toBe(false);
    expect(events).toEqual([]);
  });

  it("a_second_save_while_one_is_in_flight_waits_for_the_same_answer", async () => {
    const t = saverSetup({}, saved);
    const first = t.saver.save();
    const second = t.saver.save();
    t.answer();
    expect(await Promise.all([first, second])).toEqual([true, true]);
    expect(t.f.calls.filter((c) => c === "saveFile")).toHaveLength(1);
  });

  it("cancel_settles_a_waiting_save_false", async () => {
    const t = saverSetup({}, saved);
    const result = t.saver.save();
    t.saver.cancel();
    expect(await result).toBe(false);
  });
});

describe("what a document opens on", () => {
  const text = (over: Partial<Extract<FileState, { phase: "text" }>> = {}): FileState => ({ phase: "text", text: "one\r\ntwo", encoding: "utf-8", eol: "crlf", hash: H1, readOnly: null, ...over });
  const kept: Draft = { path: "C:\\a.txt", text: "mine", base_text: "one\ntwo", base_hash: H1, encoding: "utf-16le", eol: "crlf", edited_ms: 1 };
  const get = async () => kept;

  it("a_file_gets_its_draft_and_its_text_with_lf_endings", async () => {
    const file = text();
    expect(await opening("C:\\a.txt", () => file, get)).toEqual({ disk: { text: "one\ntwo", hash: H1 }, draft: kept });
  });
  it("a_read_only_file_gets_its_draft_too_so_the_text_typed_before_can_be_read", async () => {
    const file = text({ readOnly: "attribute" });
    expect((await opening("C:\\a.txt", () => file, get)).draft).toEqual(kept);
  });
  it("opens_on_the_disk_as_it_is_when_the_draft_has_come", async () => {
    const first = text();
    const later = text({ text: "newer", hash: "2".repeat(64) });
    expect((await opening("C:\\a.txt", () => later, get)).disk).toEqual({ text: "newer", hash: "2".repeat(64) });
  });
  it("a_file_that_was_gone_when_its_tab_opened_opens_as_no_file_with_its_draft", async () => {
    const file: FileState = { phase: "deleted" };
    expect(opensOn(file)).toBe(true);
    expect(await opening("C:\\a.txt", () => file, get)).toEqual({ disk: { text: "", hash: null }, draft: kept });
  });
  it("a_draft_says_how_its_file_was_stored_and_an_unknown_name_is_utf8_with_lf", () => {
    expect(formatOf(kept)).toEqual({ encoding: "utf-16le", eol: "crlf" });
    expect(formatOf({ encoding: "latin-1", eol: "cr" })).toEqual({ encoding: "utf-8", eol: "lf" });
  });
  it("nothing_opens_on_a_file_still_loading_or_one_that_is_not_text", () => {
    for (const file of [{ phase: "loading" }, { phase: "binary" }, { phase: "error" }] as FileState[]) expect(opensOn(file)).toBe(false);
    expect(opensOn(text())).toBe(true);
  });
});

describe("a tab that opens on a file another tab has just left", () => {
  const draft: Draft = { path: "C:\\proj\\a.txt", text: "mine", base_text: "one", base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 1 };
  const turn = async () => {
    for (let i = 0; i < 5; i++) await Promise.resolve();
  };

  it("waits_for_the_draft_the_other_tab_is_still_sending", async () => {
    const order: string[] = [];
    let answer!: (ok: boolean) => void;
    const put = () => {
      order.push("put sent");
      return new Promise<boolean>((done) => (answer = done)).then((ok) => (order.push("put answered"), ok));
    };
    const queue = createDraftQueue({ put, drop: async () => {}, failed: () => {}, busy: (work) => draftBusy(draft.path, work) });
    queue.put(() => draft);
    // The tab goes: its waiting put is sent at once.
    queue.flush();
    // The other tab names the file its own way.
    const got = draftOnceSent("c:/proj/A.TXT", async () => (order.push("get"), draft));
    await turn();
    expect(order).toEqual(["put sent"]);
    answer(true);
    expect(await got).toEqual(draft);
    expect(order).toEqual(["put sent", "put answered", "get"]);
  });

  it("asks_at_once_when_nothing_is_on_its_way_for_the_file", async () => {
    const order: string[] = [];
    const queue = createDraftQueue({ put: () => new Promise<boolean>(() => {}), drop: async () => {}, failed: () => {}, busy: (work) => draftBusy("C:\\proj\\other.txt", work) });
    queue.put(() => draft);
    queue.flush();
    await draftOnceSent(draft.path, async () => (order.push("get"), null));
    expect(order).toEqual(["get"]);
  });

  it("asks_again_without_waiting_once_the_request_is_answered", async () => {
    const queue = createDraftQueue({ put: async () => true, drop: async () => {}, failed: () => {}, busy: (work) => draftBusy(draft.path, work) });
    queue.put(() => draft);
    queue.flush();
    await turn();
    const order: string[] = [];
    void draftOnceSent(draft.path, async () => (order.push("get"), null));
    await Promise.resolve();
    expect(order).toEqual(["get"]);
  });
});
