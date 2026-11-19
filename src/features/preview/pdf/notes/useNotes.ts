import { useEffect, useImperativeHandle, useRef, useState, type Ref, type RefObject } from "react";
import { dropDraft, getPdfDraft, putPdfDraft, saveFile, unsaveFile } from "../../../editor/editorApi";
import { dirtyNews, type SaveCommands } from "../../../editor/TextTab";
import { createDraftQueue, createSaver, draftBusy, draftOnceSent } from "../../../editor/useEditorFile";
import { editorKeysAllowed, type NotesEditor } from "./editor";
import { copyName, notesReducer, unsavedNotes, type NotesEffect, type NotesEvent, type NotesState } from "./notesDoc";

/**
 * Where the keyboard is now decides whether pdf.js may take the keys: only inside the tab, with the window focused. Called on the
 * tab's focusin and focusout (a tick later, once focus has landed) and on the window's focus and blur.
 */
export function onFocusMove(editor: Pick<NotesEditor, "setLive">, root: Element | null, active: Element | null, windowFocused: boolean): void {
  editor.setLive(editorKeysAllowed(active, root, windowFocused));
}

type EscKey = { key: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean; defaultPrevented?: boolean; preventDefault(): void };

/**
 * Esc on the pages ends a note being typed or unselects the selected note first, and is then taken (default-prevented) so it
 * doesn't go on to the terminal. pdf.js never sees Esc itself (`pdfjsTakesKey`), so this is the only way it reaches the notes.
 */
export function notesEscape(e: EscKey, editor: Pick<NotesEditor, "escape"> | null): void {
  if (e.key !== "Escape" || e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || e.defaultPrevented) return;
  if (editor?.escape()) e.preventDefault();
}

/** A PDF's draft is put this long after the last change to its notes (a text draft waits `DRAFT_MS`). */
export const PDF_DRAFT_MS = 1500;

/** What a save over `SAVE_PDF_CAP` says, as a text tab's says its 2 MB. */
const tooLargeLine = (name: string) => `${name} is over 58 MB, so it can't be saved.`;
const readOnlyLine = (name: string) => `${name} is read-only, so the notes can't be saved into it.`;
/** What a serialize that threw says: pdf.js couldn't write the notes into the file's bytes. */
const unwrittenLine = (name: string) => `The notes couldn't be written into ${name}.`;
/** What "Save as a copy" says when `(notes)` to `(notes 99)` are all taken beside the file. */
const noCopyNameLine = (stem: string) => `${stem} (notes).pdf to ${stem} (notes 99).pdf are all taken, so no copy was saved.`;

/** The core calls the runner makes; a test hands it fakes. */
export type NotesApi = {
  saveFile: typeof saveFile;
  unsaveFile: typeof unsaveFile;
  putPdfDraft: typeof putPdfDraft;
  dropDraft: typeof dropDraft;
  getPdfDraft: typeof getPdfDraft;
};
const REAL_API: NotesApi = { saveFile, unsaveFile, putPdfDraft, dropDraft, getPdfDraft };

export type NotesRunnerDeps = {
  path: string;
  name: string;
  /** The tab's note editor, while there is one. */
  editor: () => Pick<NotesEditor, "save" | "markSaved" | "editing"> | null;
  /** The document to show now: a draft's bytes, or the file's after a reload. */
  load: (bytes: ArrayBuffer, dirty: boolean) => void;
  announce: (text: string) => void;
  /** Opens a copy the notes were saved in, in a new tab of the session. */
  openCopy: (path: string) => void;
  /** Told each new state, for the view to draw. */
  onState?: (s: NotesState) => void;
  readOnly?: () => boolean;
  api?: NotesApi;
  now?: () => number;
};

export type NotesRunner = {
  state(): NotesState;
  dispatch(e: NotesEvent): void;
  /** The file as the disk holds it now: `hash` null for a deleted file. A read that comes during a save waits for its answer. */
  disk(hash: string | null, bytes: ArrayBuffer | null): void;
  /** Looks for the file's draft and shows it when there is one. */
  open(): Promise<void>;
  /** The tab is gone, or is going: nothing more is shown, and a caller waiting on a save is answered false. */
  close(): void;
  /** Saves, and resolves true once the notes are in the file (see `createSaver`). */
  save(): Promise<boolean>;
  /** Drops the draft: the person chose not to keep the notes. */
  discard(): void;
  /** Sends a draft that is waiting now, unless a note is being typed (the window lost focus, or the page was hidden). */
  flush(): void;
  /**
   * The editor is going: a waiting draft is taken now, even from a note being typed, and this resolves once its bytes are taken,
   * so the editor and the document may be destroyed. The put goes on without them.
   */
  unload(): Promise<void>;
};

type Taken = { bytes: Uint8Array; baseHash: string | null; editedMs: number };

/** The state of a tab that has just opened on the file (`hash` null: the file was gone). */
export function notesStart(hash: string | null, bytes: ArrayBuffer): NotesState {
  return { base: { hash, bytes }, dirty: false, saving: false, lastSave: null, line: null, refusedSave: false, conflict: null, fromDraft: false };
}

/**
 * Runs the notes reducer's effects for one PDF tab. The state lives here so two events in one tick see each other, as in a
 * text tab's `useEditorFile`. A draft waits `PDF_DRAFT_MS` after the last change and is never taken while a note is being typed:
 * the end of the typing is a change of its own. The puts and the drop go through the text tab's queue, one at a time.
 */
export function createNotesRunner(first: { hash: string | null; bytes: ArrayBuffer }, d: NotesRunnerDeps): NotesRunner {
  const api = d.api ?? REAL_API;
  const now = d.now ?? Date.now;
  let state = notesStart(first.hash, first.bytes);
  let opens = 0;
  let closed = false;
  // The person said not to keep the notes; a new change takes that back.
  let discarded = false;
  let editedAt = now();
  // The newest draft taken, which the queue sends.
  let taken: Taken | null = null;
  let draftTimer: ReturnType<typeof setTimeout> | undefined;
  // A draft being serialized: the promise ends once its bytes are taken. `edits` counts the changes, and `takingFrom` is the
  // count the draft being serialized started at, so a change made since is known not to be in it.
  let taking: Promise<void> | null = null;
  let edits = 0;
  let takingFrom = 0;
  // Callers waiting for the save, overwrite or undo of a save in flight to be answered.
  let answered: Array<() => void> = [];
  // A draft whose serializing failed: the tab going, or the window losing focus, tries again.
  let owed = false;
  // The newest disk read that came during a save, and the file the save started from.
  let held: { hash: string | null; bytes: ArrayBuffer | null } | null = null;
  let savedFrom: string | null = null;
  let refusal = "";

  const queue = createDraftQueue<Taken>({
    put: async (t) => {
      const r = await api.putPdfDraft(d.path, t.bytes, t.baseHash, t.editedMs);
      if (r === "kept") return true;
      refusal = r.refused;
      return false;
    },
    drop: () => api.dropDraft(d.path),
    failed: () => dispatch({ type: "draftRefused", line: refusal }),
    busy: (work) => draftBusy(d.path, work),
    delay: 0,
  });
  const saver = createSaver<NotesState>({ doc: () => state, dispatch: (e) => dispatch(e), readOnly: () => d.readOnly?.() ?? false });

  const stopTimer = () => {
    clearTimeout(draftTimer);
    draftTimer = undefined;
  };
  // `kept`: the editor of a tab that is going, which the view has already let go of.
  const takeDraft = (kept?: ReturnType<NotesRunnerDeps["editor"]>): Promise<void> => {
    stopTimer();
    if (taking) {
      // A change made while this draft serializes isn't in it: once it ends, the change is taken too.
      const from = takingFrom;
      return taking.then(() => (edits !== from ? takeDraft(kept) : undefined));
    }
    const editor = kept ?? d.editor();
    if (!editor || discarded) return Promise.resolve();
    takingFrom = edits;
    const at = editedAt;
    const work = editor.save().then(
      (bytes) => {
        taking = null;
        owed = false;
        taken = { bytes, baseHash: state.base.hash, editedMs: at };
        dispatch({ type: "serialized", bytes, for: "draft" });
      },
      () => {
        taking = null;
        owed = true;
      },
    );
    taking = work;
    return work;
  };
  const scheduleDraft = () => {
    stopTimer();
    draftTimer = setTimeout(() => {
      draftTimer = undefined;
      // The note's commit when the typing ends asks again.
      if (d.editor()?.editing()) return;
      void takeDraft();
    }, PDF_DRAFT_MS);
  };

  const serializeFor = (then: "save" | "copy" | "overwrite") => {
    const editor = d.editor();
    const failed = () => dispatch({ type: "saveRefused", line: unwrittenLine(d.name) });
    if (!editor) return failed();
    // pdf.js takes the notes as `save` is called, after ending the typing in a note: a change counted from here isn't in the bytes.
    const serializing = editor.save();
    const from = edits;
    serializing.then((bytes) => dispatch({ type: "serialized", bytes, for: then, later: edits !== from }), failed);
  };

  const runSave = async (bytes: Uint8Array, expect: string | null) => {
    const r = await api.saveFile(d.path, bytes, expect);
    switch (r.kind) {
      case "saved":
        return dispatch({ type: "saved", saved: r.saved, bytes });
      case "conflict":
        return dispatch({ type: "saveConflict", hash: r.hash });
      case "tooLarge":
        return dispatch({ type: "saveRefused", line: tooLargeLine(d.name) });
      case "refused":
        return dispatch({ type: "saveRefused", line: r.line });
    }
  };

  const runCopy = async (bytes: Uint8Array) => {
    const cut = Math.max(d.path.lastIndexOf("/"), d.path.lastIndexOf("\\"));
    const folder = d.path.slice(0, cut + 1);
    const stem = d.name.replace(/\.pdf$/i, "");
    // A name another file has; Windows folds case, so the look-up does too.
    const tried = new Set<string>();
    for (;;) {
      const name = copyName(stem, (n) => tried.has(n.toLowerCase()));
      if (name === null) return dispatch({ type: "saveRefused", line: noCopyNameLine(stem) });
      const r = await api.saveFile(folder + name, bytes, null);
      switch (r.kind) {
        case "saved":
          dispatch({ type: "copied", name });
          if (!closed) d.openCopy(folder + name);
          return;
        case "conflict":
          tried.add(name.toLowerCase());
          continue;
        case "tooLarge":
          return dispatch({ type: "saveRefused", line: tooLargeLine(name) });
        case "refused":
          return dispatch({ type: "saveRefused", line: r.line });
      }
    }
  };

  const runUnsave = async (snapshot: string) => {
    const r = await api.unsaveFile(d.path, snapshot);
    if (r.kind === "saved") dispatch({ type: "unsaved", hash: r.saved.hash });
    else if (r.kind === "conflict") dispatch({ type: "unsaveRefused" });
    else dispatch({ type: "unsaveFailed", line: r.line });
  };

  const run = (effect: NotesEffect) => {
    switch (effect.type) {
      case "serialize":
        if (effect.then === "draft") return scheduleDraft();
        return serializeFor(effect.then);
      case "save":
        return void runSave(effect.bytes, effect.expect);
      case "saveCopy":
        return void runCopy(effect.bytes);
      case "unsave":
        return void runUnsave(effect.snapshot);
      case "putDraft": {
        const t = taken;
        if (discarded || t === null) return;
        queue.put(() => t);
        return queue.flush();
      }
      case "dropDraft":
        stopTimer();
        owed = false;
        return queue.drop();
      case "load":
        if (!closed) d.load(effect.bytes, effect.dirty);
        return;
      case "markSaved":
        return d.editor()?.markSaved();
      case "announce":
        return d.announce(effect.text);
    }
  };

  function dispatch(e: NotesEvent): void {
    if (e.type === "edited") {
      discarded = false;
      editedAt = now();
      edits++;
    }
    const wasSaving = state.saving;
    const out = notesReducer(state, e, d.name);
    state = out.state;
    if (!wasSaving && state.saving) savedFrom = state.base.hash;
    for (const effect of out.effects) run(effect);
    if (wasSaving && !state.saving) {
      const waiting = answered;
      answered = [];
      for (const resume of waiting) resume();
    }
    if (wasSaving && !state.saving && held !== null) {
      // The save is answered: a read from before it is the file it replaced, and old news; a newer one is somebody else's.
      const read = held;
      held = null;
      if (read.hash !== savedFrom) return dispatch({ type: "disk", hash: read.hash, bytes: read.bytes });
    }
    d.onState?.(state);
    saver.observe(state);
  }

  return {
    state: () => state,
    dispatch,
    disk(hash, bytes) {
      if (state.saving) held = { hash, bytes };
      else dispatch({ type: "disk", hash, bytes });
    },
    async open() {
      const mine = ++opens;
      closed = false;
      const draft = await draftOnceSent(d.path, api.getPdfDraft);
      if (mine !== opens || draft === null) return;
      dispatch({ type: "draft", bytes: draft.bytes, baseHash: draft.baseHash });
    },
    close() {
      opens++;
      closed = true;
      saver.cancel();
    },
    save() {
      // The shared saver answers a read-only file with a quiet false; unsaved notes get the line and Save as a copy instead.
      if (d.readOnly?.() && state.dirty && !state.saving && state.conflict === null) {
        dispatch({ type: "saveRefused", line: readOnlyLine(d.name) });
        return Promise.resolve(false);
      }
      return saver.save();
    },
    discard() {
      discarded = true;
      stopTimer();
      owed = false;
      queue.drop();
    },
    flush() {
      if ((draftTimer === undefined && !owed) || d.editor()?.editing()) return;
      void takeDraft().then(() => queue.flush());
    },
    async unload() {
      // A save, an overwrite or an undo of a save in flight decides whether the notes are in the file: the editor waits for its
      // answer, which asks for a draft when they aren't.
      // The view lets go of the editor as it calls this, so the draft is taken from the editor it had.
      const editor = d.editor();
      if (state.saving) await new Promise<void>((resume) => answered.push(resume));
      if (draftTimer !== undefined || taking || owed) await takeDraft(editor);
      queue.flush();
    },
  };
}

export type NotesOptions = {
  path: string;
  name: string;
  /** The file as the tab has it now: `hash` null once it is deleted. */
  hash: string | null;
  bytes: ArrayBuffer;
  readOnly: boolean;
  /** The PDF can't take notes (`notesBlocked`). Its save commands are then handed out only while it still holds unsaved notes, so the close question can discard them. */
  blocked: boolean;
  announce?: (text: string) => void;
  onDirty?: (path: string, dirty: boolean) => void;
  openCopy: (path: string) => void;
  /** Gets the tab's save commands: Ctrl+S, Undo save and the close question. */
  save?: Ref<SaveCommands | null>;
  /** Puts the keyboard on the pages. */
  focus: () => void;
};

export type Notes = {
  /** What the view shows: null until the draft has been looked for, so a draft's notes don't come in after the file. */
  shown: { bytes: ArrayBuffer; version: number } | null;
  state: NotesState;
  runner: NotesRunner;
  /** Where the view keeps its note editor, which the runner serializes with. */
  editor: RefObject<NotesEditor | null>;
};

/** A PDF tab's notes: the runner for its file, the document to show, and the save commands. One per file (the view is keyed by path). */
export function useNotes(o: NotesOptions): Notes {
  const latest = useRef(o);
  latest.current = o;
  const editor = useRef<NotesEditor | null>(null);
  const version = useRef(0);
  const [shown, setShown] = useState<Notes["shown"]>(null);
  const [runner] = useState(() =>
    createNotesRunner(
      { hash: o.hash, bytes: o.bytes },
      {
        path: o.path,
        name: o.name,
        editor: () => editor.current,
        load: (bytes) => setShown({ bytes, version: ++version.current }),
        announce: (text) => latest.current.announce?.(text),
        openCopy: (path) => latest.current.openCopy(path),
        onState: (s) => setState(s),
        readOnly: () => latest.current.readOnly,
      },
    ),
  );
  const [state, setState] = useState<NotesState>(() => runner.state());

  useEffect(() => {
    let live = true;
    void runner.open().then(() => {
      if (live) setShown((s) => s ?? { bytes: runner.state().base.bytes, version: ++version.current });
    });
    return () => {
      live = false;
      runner.close();
    };
  }, [runner]);

  // Each new read of the file; the first is the one the runner started from.
  useEffect(() => {
    runner.disk(o.hash, o.hash === null ? null : o.bytes);
  }, [runner, o.hash, o.bytes]);

  // A draft waiting when the window loses focus or the app is hidden goes now: the app may be closed next.
  useEffect(() => {
    const away = () => runner.flush();
    const hidden = () => document.visibilityState === "hidden" && runner.flush();
    window.addEventListener("blur", away);
    document.addEventListener("visibilitychange", hidden);
    return () => {
      window.removeEventListener("blur", away);
      document.removeEventListener("visibilitychange", hidden);
    };
  }, [runner]);

  // The dot, said once the draft has been looked for, as a text tab says it once its document opens. It stays while a save
  // is in flight, so a tab closed then is asked about.
  const saidDirty = useRef<boolean | null>(null);
  const ready = shown !== null;
  const unsaved = unsavedNotes(state);
  useEffect(() => {
    const news = dirtyNews(saidDirty.current, ready, unsaved);
    if (news === null) return;
    saidDirty.current = news;
    latest.current.onDirty?.(o.path, news);
  }, [ready, unsaved, o.path]);

  const hidden = o.blocked && !unsaved;
  useImperativeHandle(
    hidden ? null : o.save,
    () => ({
      save: () => runner.save(),
      undoSave() {
        const s = runner.state();
        if (s.lastSave === null || s.saving) return false;
        runner.dispatch({ type: "unsave" });
        return true;
      },
      canUndoSave: () => runner.state().lastSave !== null && !runner.state().saving,
      discard: () => runner.discard(),
      dirty: () => unsavedNotes(runner.state()),
      focus: () => latest.current.focus(),
    }),
    [runner, hidden],
  );

  return { shown, state, runner, editor };
}
