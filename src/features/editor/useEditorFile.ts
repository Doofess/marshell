import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import type { Draft } from "../../generated/Draft";
import { splitPath } from "../preview/kinds";
import type { FileState } from "../preview/usePreviewFile";
import type { EditorHandle } from "./Editor";
import { start, step, type Disk, type DocEvent, type DocState, type Effect } from "./editorDoc";
import { dropDraft, getDraft, mergeTexts, putDraft, saveFile, unsaveFile } from "./editorApi";
import { encodeFile, type Encoding, type Eol } from "./textFile";

/** A draft is written this long after the last key. */
export const DRAFT_MS = 400;

/** What the editor reads a file's text as: `\n` between lines, whatever the file uses. */
export const lf = (text: string) => text.replace(/\r\n|\r/g, "\n");

/**
 * The draft requests, one at a time and in order. A put waits `DRAFT_MS` after the last one, and sends whatever text is newest
 * when it goes. A drop cancels a put that has not gone out, and goes out after one that is on the wire, so no put ever lands
 * after the drop that followed it.
 */
export type DraftQueue = {
  /** Asks for a draft to be kept; `make` is read when the request is sent. */
  put(make: () => Draft): void;
  /** Sends a waiting put now. */
  flush(): void;
  /** Cancels a waiting put and drops the draft. */
  drop(): void;
};

// The draft requests on their way to the core, by file. A session draws one editor for a file, and the tab that takes over opens on
// the draft the other left: it must not read the draft before the other's last put has landed. The key folds case and slashes on
// every system; two files that differ only there wait for each other, which costs nothing.
const onTheWay = new Map<string, Promise<void>>();
const fileKey = (path: string): string => path.replace(/\\/g, "/").toLowerCase();

/** Tells that a draft request for the file is on its way; `work` ends when the core has answered it. */
export function draftBusy(path: string, work: Promise<void>): void {
  const key = fileKey(path);
  onTheWay.set(key, work);
  void work.then(() => {
    if (onTheWay.get(key) === work) onTheWay.delete(key);
  });
}

/** The file's draft, asked for once every draft request for the file has been answered. */
export async function draftOnceSent(path: string, get: (path: string) => Promise<Draft | null>): Promise<Draft | null> {
  await onTheWay.get(fileKey(path));
  return get(path);
}

export function createDraftQueue(o: { put: (d: Draft) => Promise<boolean>; drop: () => Promise<void>; failed: () => void; busy?: (work: Promise<void>) => void; delay?: number }): DraftQueue {
  const delay = o.delay ?? DRAFT_MS;
  let timer: ReturnType<typeof setTimeout> | undefined;
  // The newest put asked for and not yet sent.
  let wanted: (() => Draft) | null = null;
  // A send is in the chain and has not started; a drop makes it void.
  let queued = false;
  let dropQueued = false;
  let epoch = 0;
  // Failed once, and no put has worked since: the person was told.
  let failing = false;
  let tail: Promise<void> = Promise.resolve();
  const chain = (job: () => Promise<void>) => {
    tail = tail.then(job).catch(() => {});
    o.busy?.(tail);
  };

  const send = async (mine: number) => {
    if (mine !== epoch) return;
    queued = false;
    const make = wanted;
    wanted = null;
    if (!make) return;
    if (await o.put(make())) failing = false;
    else if (!failing) {
      failing = true;
      o.failed();
    }
  };
  const fire = () => {
    timer = undefined;
    if (wanted === null || queued) return;
    queued = true;
    dropQueued = false;
    const mine = epoch;
    chain(() => send(mine));
  };
  return {
    put(make) {
      wanted = make;
      // A send already in the chain takes the newest one when it starts.
      if (queued) return;
      clearTimeout(timer);
      timer = setTimeout(fire, delay);
    },
    flush() {
      clearTimeout(timer);
      fire();
    },
    drop() {
      clearTimeout(timer);
      timer = undefined;
      wanted = null;
      queued = false;
      epoch++;
      if (dropQueued) return;
      dropQueued = true;
      chain(async () => {
        dropQueued = false;
        await o.drop();
      });
    },
  };
}

/**
 * What `save()` answers a caller that waits for it (the close question). It asks the reducer for a save and then holds the
 * promise until the document stops saving: `true` when the file is then saved, `false` for a refused, conflicting or too large
 * save, a bar that is up, a read-only file, or a tab that went. The reducer stays pure; `observe` is told each new state.
 */
export type Saver = {
  save(): Promise<boolean>;
  /** Tells the saver the state after a step. */
  observe(s: DocState): void;
  /** Settles every waiting save `false`: the tab went. */
  cancel(): void;
};

export function createSaver(o: { doc: () => DocState; dispatch: (e: DocEvent) => void; readOnly: () => boolean; /** The document has been opened. Absent: it has. */ open?: () => boolean }): Saver {
  let waiting: Array<(saved: boolean) => void> = [];
  const answer = (s: DocState) => !s.dirty && s.conflict === null;
  const settle = (saved: (s: DocState) => boolean, s: DocState) => {
    const now = waiting;
    waiting = [];
    for (const resolve of now) resolve(saved(s));
  };
  return {
    save() {
      // Before the document is open the state is the empty start, which a save would send as an empty file.
      if (o.readOnly() || o.open?.() === false) return Promise.resolve(false);
      o.dispatch({ type: "save" });
      const s = o.doc();
      // No save started and none is in flight: the reducer said there is nothing to save, or that it can't be.
      if (!s.saving) return Promise.resolve(answer(s));
      return new Promise<boolean>((resolve) => waiting.push(resolve));
    },
    observe(s) {
      if (!s.saving && waiting.length > 0) settle(answer, s);
    },
    cancel() {
      settle(() => false, o.doc());
    },
  };
}

/** Everything the effects touch, so the runner is a plain function and a test hands it fakes. */
export type EffectDeps = {
  path: string;
  name: string;
  /** The state after the step that asked for the effect, or newer. */
  doc: () => DocState;
  /** How the file is stored: what a save and a draft write it as. */
  format: () => { encoding: Encoding; eol: Eol };
  editor: () => EditorHandle | null;
  announce: (text: string) => void;
  /** The keyboard is in this tab, and the tab is the one on screen. */
  reading: () => boolean;
  refetch: () => void;
  dispatch: (e: DocEvent) => void;
  drafts: DraftQueue;
  now: () => number;
  /** The tab's document is no longer on screen: it was unmounted. */
  gone: () => boolean;
  api: { saveFile: typeof saveFile; unsaveFile: typeof unsaveFile; mergeTexts: typeof mergeTexts };
};

async function runSave(effect: Extract<Effect, { type: "save" }>, d: EffectDeps): Promise<void> {
  const s = d.doc();
  const { encoding, eol } = d.format();
  // The text the reducer says the save sent; the base becomes exactly this when the save is answered.
  const bytes = encodeFile(s.sent ?? s.text, encoding, eol);
  const r = await d.api.saveFile(d.path, bytes, effect.expect);
  switch (r.kind) {
    case "saved":
      d.dispatch({ type: "saved", hash: r.saved.hash, snapshot: r.saved.snapshot });
      return;
    case "conflict":
      d.dispatch({ type: "saveConflict", hash: r.hash });
      return;
    case "tooLarge":
      d.dispatch({ type: "saveTooLarge" });
      return;
    case "refused":
      d.dispatch({ type: "saveFailed", line: r.line });
      return;
  }
}

async function runUnsave(effect: Extract<Effect, { type: "unsave" }>, d: EffectDeps): Promise<void> {
  const r = await d.api.unsaveFile(d.path, effect.snapshot);
  if (r.kind === "saved") d.dispatch({ type: "unsaved", hash: r.saved.hash });
  else if (r.kind === "conflict") d.dispatch({ type: "unsaveRefused" });
  else d.dispatch({ type: "unsaveFailed", line: r.line });
}

async function runMerge(effect: Extract<Effect, { type: "merge" }>, d: EffectDeps): Promise<void> {
  const r = await d.api.mergeTexts(effect.base, effect.mine, effect.disk.text);
  // The tab went while the merge ran (closed, or its session put away): an answer taken now would write a draft the person
  // may just have said not to keep, and announce a merge of a tab that is not there.
  if (d.gone()) return;
  if (r.kind === "merged") d.dispatch({ type: "merged", id: effect.id, text: r.text, conflicts: r.conflicts, mine: effect.mine, disk: effect.disk.hash });
  else d.dispatch({ type: "mergeFailed", id: effect.id, line: r.line });
}

/** Runs one effect of the reducer. The save, unsave and merge return a promise that settles when their answer has been dispatched. */
export function runEffect(effect: Effect, d: EffectDeps): Promise<void> | void {
  switch (effect.type) {
    case "setText":
      d.editor()?.setText(effect.text, effect.marks);
      return;
    case "putDraft": {
      const at = d.now();
      d.drafts.put(() => {
        const s = d.doc();
        const { encoding, eol } = d.format();
        return { path: d.path, text: s.text, base_text: s.base.text, base_hash: s.base.hash, encoding, eol, edited_ms: at };
      });
      return;
    }
    case "dropDraft":
      d.drafts.drop();
      return;
    case "save":
      return runSave(effect, d);
    case "unsave":
      return runUnsave(effect, d);
    case "refetch":
      d.refetch();
      return;
    case "merge":
      return runMerge(effect, d);
    case "showConflict":
      d.editor()?.toConflict("first");
      return;
    case "announce":
      if (effect.reading && !d.reading()) return;
      d.announce(effect.text);
      return;
  }
}

const ENCODINGS: readonly Encoding[] = ["utf-8", "utf-8-bom", "utf-16le", "utf-16be"];

/** How a draft says its file was stored; anything it doesn't name plainly is UTF-8 with LF endings. */
export function formatOf(draft: Pick<Draft, "encoding" | "eol">): { encoding: Encoding; eol: Eol } {
  return { encoding: ENCODINGS.find((e) => e === draft.encoding) ?? "utf-8", eol: draft.eol === "crlf" ? "crlf" : "lf" };
}

/** A state of the file a document can open on: its text, or a file that is gone and may have left a draft. */
export const opensOn = (file: FileState): boolean => file.phase === "text" || file.phase === "deleted";

/**
 * What a document opens on: the draft kept for the file, and the disk as it is once the draft has come (the file may have moved
 * on while it was fetched). A file that is gone opens as no file. A read-only file's draft is read like any other: the text
 * typed before the file became read-only shows in the editor, where it can be read and copied.
 */
export async function opening(path: string, latest: () => FileState, get: (path: string) => Promise<Draft | null>): Promise<{ disk: Disk; draft: Draft | null }> {
  const draft = await get(path);
  const at = latest();
  return { disk: at.phase === "text" ? { text: lf(at.text), hash: at.hash } : { text: "", hash: null }, draft };
}

export type EditorFile = {
  doc: DocState;
  ready: boolean;
  dispatch: (e: DocEvent) => void;
  /** Saves and resolves when the save has been answered: true when the file is then saved (see `createSaver`). */
  save: () => Promise<boolean>;
  /** Drops the draft through the tab's own queue, so the flush when the tab goes cannot write it back. */
  discard: () => void;
};

/**
 * A text tab's document. The first text of the file fetches its draft and opens the document; after that each new text of the
 * file is a `disk` event, and a file that went is `gone`. The state is the reducer's (editorDoc.ts), kept in a ref so that two
 * events in one tick see each other, and the effects it asks for run here, once per step.
 */
export function useEditorFile(path: string, file: FileState, o: { announce: (text: string) => void; reading: () => boolean; refetch: () => void; editor: RefObject<EditorHandle | null>; readOnly: () => boolean }): EditorFile {
  const name = splitPath(path).name;
  const docRef = useRef<DocState>(start);
  const [doc, setDoc] = useState<DocState>(start);
  const [ready, setReady] = useState(false);
  const format = useRef<{ encoding: Encoding; eol: Eol }>({ encoding: "utf-8", eol: "lf" });
  const deps = useRef<EffectDeps>(null as unknown as EffectDeps);
  // What the draft queue says on a failure, and where it writes, as of the last render.
  const live = useRef({ path, name });
  live.current = { path, name };
  const drafts = useRef<DraftQueue | null>(null);
  drafts.current ??= createDraftQueue({
    put: putDraft,
    drop: () => dropDraft(live.current.path),
    failed: () => deps.current.announce(`The unsaved edits to ${live.current.name} couldn't be kept as a draft.`),
    busy: (work) => draftBusy(live.current.path, work),
  });

  const saver = useRef<Saver | null>(null);
  const dispatch = useCallback((e: DocEvent) => {
    const d = deps.current;
    const out = step(docRef.current, e, d.name);
    docRef.current = out.state;
    setDoc(out.state);
    saver.current?.observe(out.state);
    for (const effect of out.effects) void runEffect(effect, d);
  }, []);
  const readOnly = useRef(o.readOnly);
  readOnly.current = o.readOnly;
  const phase = useRef<"waiting" | "opening" | "open">("waiting");
  saver.current ??= createSaver({ doc: () => docRef.current, dispatch, readOnly: () => readOnly.current(), open: () => phase.current === "open" });
  const gone = useRef(false);
  deps.current = {
    path,
    name,
    doc: () => docRef.current,
    format: () => format.current,
    editor: () => o.editor.current,
    announce: o.announce,
    reading: o.reading,
    refetch: o.refetch,
    dispatch,
    drafts: drafts.current,
    now: () => Date.now(),
    gone: () => gone.current,
    api: { saveFile, unsaveFile, mergeTexts },
  };

  const latest = useRef(file);
  latest.current = file;
  useEffect(() => {
    gone.current = false;
    return () => {
      gone.current = true;
      saver.current?.cancel();
    };
  }, []);

  useEffect(() => {
    if (file.phase === "text") format.current = { encoding: file.encoding, eol: file.eol };
    if (phase.current === "waiting") {
      if (!opensOn(file)) return;
      phase.current = "opening";
      void opening(path, () => latest.current, (p) => draftOnceSent(p, getDraft)).then((found) => {
        if (gone.current) return;
        // A file that is gone is written again the way its draft says it was stored.
        if (latest.current.phase !== "text" && found.draft) format.current = formatOf(found.draft);
        phase.current = "open";
        dispatch({ type: "opened", disk: found.disk, draft: found.draft });
        setReady(true);
      });
    } else if (phase.current === "open") {
      // Every new text of the file goes to the reducer: it knows an unchanged one, and learns the text an undo put back.
      if (file.phase === "text") dispatch({ type: "disk", disk: { text: lf(file.text), hash: file.hash } });
      else if (file.phase === "deleted") dispatch({ type: "gone" });
    }
  }, [file]);

  // A put that is waiting goes out when the window is hidden or loses focus, and when the tab goes.
  useEffect(() => {
    const send = () => drafts.current?.flush();
    const hidden = () => document.visibilityState === "hidden" && send();
    document.addEventListener("visibilitychange", hidden);
    window.addEventListener("blur", send);
    return () => {
      document.removeEventListener("visibilitychange", hidden);
      window.removeEventListener("blur", send);
      send();
    };
  }, []);

  const save = useCallback(() => saver.current!.save(), []);
  const discard = useCallback(() => drafts.current?.drop(), []);
  return { doc, ready, dispatch, save, discard };
}
