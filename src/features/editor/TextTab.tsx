import { useCallback, useEffect, useId, useImperativeHandle, useLayoutEffect, useRef, useState, type Ref } from "react";
import type { PaneEntry } from "../../generated/PaneEntry";
import { findCommand } from "../../lib/bindings";
import type { Os } from "../../lib/keymap";
import { HtmlView } from "../preview/HtmlView";
import type { PreviewKind } from "../preview/kinds";
import { splitPath } from "../preview/kinds";
import type { LinkTarget } from "../preview/links";
import { MarkdownView } from "../preview/MarkdownView";
import type { ReadOnlyReason } from "../preview/reload";
import type { FileState } from "../preview/usePreviewFile";
import { ChangedBar } from "./ChangedBar";
import { Editor, type EditorHandle } from "./Editor";
import type { DocEvent, DocState } from "./editorDoc";
import { EditorFooter } from "./EditorFooter";
import { languageFor } from "./fileTypes";
import { COLOUR_CAP } from "./tokens";
import { lf, useEditorFile } from "./useEditorFile";
import type { Encoding, Eol } from "./textFile";

/** What the pane can ask of a text tab: the palette's commands and the Ctrl+S key. */
export type TextTabCommands = {
  /** Saves. Resolves when the save has been answered: true when the file is then saved, false when it was refused, conflicted, the bar is up or the file is read-only. True at once with nothing to save. */
  save(): Promise<boolean>;
  /** Undoes the last save. False when there is none to undo. */
  undoSave(): boolean;
  /** Whether `undoSave` can run now: a save to undo, and none in flight. */
  canUndoSave(): boolean;
  /** Drops the file's draft through the tab's own queue. The tab is about to close; nothing may write the draft back. */
  discard(): void;
  goToLine(): void;
  toggleWrap(): void;
  /** Moves to the next conflict marker in the text; does nothing while the Rendered view hides the editor. */
  nextConflict(): void;
  /** Moves to the previous conflict marker; does nothing while the Rendered view hides the editor. */
  previousConflict(): void;
  dirty(): boolean;
  /** Puts the keyboard in the editor: where focus goes back to when a save the close question asked for did not go through. */
  focus(): void;
};

/**
 * Ctrl+S, or Cmd+S on macOS. The letter is what the key types where the layout is Latin, and the key where S is printed where
 * it is not (a Hebrew or Cyrillic layout types no "s").
 */
export function isSaveKey(e: { key: string; code: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }, os: Os): boolean {
  const mod = os === "mac" ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  if (!mod || e.altKey || e.shiftKey) return false;
  return /^[a-z]$/i.test(e.key) ? e.key.toLowerCase() === "s" : e.code === "KeyS";
}

/**
 * What a tab tells the window about its unsaved edits: `dirty` once the document is open, and after that on each change. Null
 * when there is nothing new to say. `told` is what it last said, null for nothing yet.
 */
export function dirtyNews(told: boolean | null, ready: boolean, dirty: boolean): boolean | null {
  return !ready || told === dirty ? null : dirty;
}

/** How long the rendered view waits after the last key. */
export const RENDER_MS = 150;

/** Word wrap starts on for prose and plain text, and off for code and markup: also code that only the file's first line names. */
export function wrapAtStart(kind: PreviewKind, path: string, firstLine: string | undefined): boolean {
  if (kind === "markdown") return true;
  return kind === "text" && languageFor(path, firstLine) === undefined;
}

/** Why a file is read-only, in the plan's words. */
export function readOnlyLine(reason: ReadOnlyReason, name: string): string {
  switch (reason) {
    case "own":
      return "Marshell doesn't save files in its own folder.";
    case "managed":
      return "This file is managed by your organization, so Marshell doesn't save it.";
    case "attribute":
      return `${name} is read-only on disk.`;
  }
}

type Focusable = { focus(): void };

/**
 * Where the keyboard goes after a button that goes away was pressed (Undo, Keep mine, Take disk): the editor, or the tab's body
 * while the Rendered view hides the editor. Left alone, focus would fall to the page with the button.
 */
export function keyboardTo(o: { rendered: boolean; editor: Focusable | null; body: Focusable | null }): void {
  if (!o.rendered && o.editor) o.editor.focus();
  else o.body?.focus();
}

/**
 * Next conflict and Previous conflict from the palette. The Rendered view hides the editor, and a command that did nothing
 * there would look broken, so it says where conflicts show.
 */
export function conflictJump(
  o: { rendered: boolean; editor: { toConflict(which: "next" | "previous"): void } | null; announce: (text: string) => void },
  which: "next" | "previous",
): void {
  if (o.rendered) o.announce("Conflicts show in the Source view.");
  else o.editor?.toConflict(which);
}

type ConflictJumper = { toConflict(which: "first"): void };

/**
 * After a merge that left conflicts, the editor goes to the first one. The Rendered view hides the editor, so the tab shows its
 * Source view first, where the merge's line about F7 and Ctrl+Z is true (6q-4b decision 7.2). True when the jump waits for it.
 */
export function firstConflict(o: { rendered: boolean; editor: ConflictJumper | null; toSource: () => void }): boolean {
  if (!o.rendered) {
    o.editor?.toConflict("first");
    return false;
  }
  o.toSource();
  return true;
}

/**
 * The jump that waited, once the Source view shows. Merge pressed behind the Rendered view left the keyboard on the tab's body,
 * so it goes on into the editor, where F7 works; anywhere else it stays where the person put it.
 */
export function sourceShown(o: { editor: (ConflictJumper & Focusable) | null; onBody: boolean }): void {
  if (o.onBody) o.editor?.focus();
  o.editor?.toConflict("first");
}

type Outbound = Exclude<LinkTarget, { kind: "anchor" } | { kind: "none" }>;
type KeyHandler = (e: KeyboardEvent) => void;

export type TextTabViewProps = {
  entry: PaneEntry;
  name: string;
  kind: PreviewKind;
  doc: DocState;
  /** The editor is made once the draft has been looked for. */
  ready: boolean;
  /** The file is known to be gone, whether or not the document has caught up. */
  gone: boolean;
  readOnly: ReadOnlyReason | null;
  encoding: Encoding;
  eol: Eol;
  cursor: { line: number; column: number };
  wrap: boolean;
  hintId: string;
  /** The text the rendered view shows, when it is not the document's. */
  rendered?: string;
  firstLine?: string;
  editor: Ref<EditorHandle> | null;
  /** The tab's own element, for the pane's body around it. */
  root?: Ref<HTMLDivElement>;
  onEdit: (text: string) => void;
  onCursor: (line: number, column: number) => void;
  appChord?: (e: KeyboardEvent) => boolean;
  onMerge: () => void;
  /** The editor's answer to a jump to a conflict: its place from 1 and the count, or 0 and 0 for none. */
  onConflict: (at: number, of: number) => void;
  onKeep: () => void;
  onTake: () => void;
  onUndo: () => void;
  onLink: (link: Outbound) => void;
  onKey: KeyHandler;
};

/** What a text tab shows for a document, top to bottom. Pure: the state comes in as props. */
export function TextTabView(p: TextTabViewProps) {
  const { doc, name, entry } = p;
  const paged = p.kind === "markdown" || p.kind === "html";
  const showRendered = paged && entry.view === "rendered";
  const notice = p.gone || doc.deleted ? `${name} was deleted on disk. Ctrl+S saves it again.` : p.readOnly !== null ? readOnlyLine(p.readOnly, name) : null;
  return (
    <div className="text-tab" ref={p.root}>
      {notice !== null && <div className="text-tab__notice">{notice}</div>}
      {doc.conflict !== null && <ChangedBar name={name} onMerge={p.readOnly === null ? p.onMerge : undefined} onKeep={p.onKeep} onTake={p.onTake} />}
      <div className="text-tab__body">
        {paged && (
          <div className="text-tab__pane" data-pane="rendered" hidden={!showRendered}>
            {p.kind === "markdown" ? <MarkdownView text={p.rendered ?? doc.text} docPath={entry.path} onLink={p.onLink} /> : <HtmlView text={p.rendered ?? doc.text} name={name} docPath={entry.path} onLink={p.onLink} onKey={p.onKey} />}
          </div>
        )}
        <div className="text-tab__pane" data-pane="editor" hidden={showRendered}>
          {doc.text.length > COLOUR_CAP && <p className="preview__note">Syntax colours are off for files over 1 MB.</p>}
          {p.ready && (
            <Editor
              ref={p.editor ?? undefined}
              name={name}
              path={entry.path}
              initial={doc.text}
              firstLine={p.firstLine}
              readOnly={p.readOnly !== null}
              wrap={p.wrap}
              onEdit={p.onEdit}
              onCursor={p.onCursor}
              onConflict={p.onConflict}
              appChord={p.appChord}
              hintId={p.hintId}
            />
          )}
        </div>
      </div>
      {/* Under the text, so a save does not move the lines the person is reading. */}
      {doc.line !== null && (
        <div className="text-tab__status">
          <span className="text-tab__line">{doc.line}</span>
          {doc.undoOffered && doc.lastSave !== null && (
            <button type="button" className="text-tab__button" onClick={p.onUndo}>
              Undo
            </button>
          )}
        </div>
      )}
      <div className="text-tab__footer" hidden={showRendered}>
        <EditorFooter line={p.cursor.line} column={p.cursor.column} encoding={p.encoding} eol={p.eol} hintId={p.hintId} />
      </div>
    </div>
  );
}

type Props = {
  entry: PaneEntry;
  file: Extract<FileState, { phase: "text" | "deleted" }>;
  kind: PreviewKind;
  /** The tab is the selected one. A kept tab that isn't has nothing on screen to mark or undo. */
  active: boolean;
  os: Os;
  /** Each change of whether the file has unsaved edits. */
  onDirty: (path: string, dirty: boolean) => void;
  commands: Ref<TextTabCommands>;
  announce: (text: string) => void;
  /** Reads the file again now. */
  refetch: () => void;
  onLink: (link: Outbound) => void;
  onKey: KeyHandler;
  /** Shows the tab's Source view: after a merge that left conflicts behind the Rendered view (6q-4b decision 7.2). */
  onSource: () => void;
};

/** The file a text tab last had text of. A deleted file keeps showing what it was. */
function textOf(file: Props["file"]): Extract<FileState, { phase: "text" }> | null {
  if (file.phase === "text") return file;
  return file.last?.phase === "text" ? file.last : null;
}

/**
 * A text file as an editor: the document is the reducer's (editorDoc.ts) and the file's effects run in `useEditorFile`. This
 * component holds the pieces that live as long as the tab: the cursor shown in the footer, the rendered view's delayed text,
 * and what the pane can call.
 */
export function TextTab({ entry, file, kind, active, os, onDirty, commands, announce, refetch, onLink, onKey, onSource }: Props) {
  const name = splitPath(entry.path).name;
  const editor = useRef<EditorHandle>(null);
  const root = useRef<HTMLDivElement>(null);
  const text = textOf(file);
  const readOnly = text?.readOnly ?? null;
  const readOnlyNow = useRef(readOnly);
  readOnlyNow.current = readOnly;
  // Someone is reading here when the keyboard is in this tab's panel: a reload is spoken to them, and to nobody typing elsewhere.
  const reading = useCallback(() => (root.current?.closest('[role="tabpanel"]') ?? root.current)?.contains(document.activeElement) ?? false, []);
  const paged = kind === "markdown" || kind === "html";
  // The Rendered view hides the editor: a jump to a conflict has nothing to move, and a merge's Ctrl+Z nothing to undo.
  const showRendered = paged && entry.view === "rendered";
  const hidden = useRef(false);
  hidden.current = showRendered;
  const toSource = useRef(onSource);
  toSource.current = onSource;
  // A jump to the first conflict that waits for the Source view a merge asked for.
  const waiting = useRef(false);
  const toFirstConflict = useCallback(() => {
    waiting.current = firstConflict({ rendered: hidden.current, editor: editor.current, toSource: () => toSource.current() });
  }, []);
  const { doc, ready, dispatch, save, discard } = useEditorFile(entry.path, file, {
    announce,
    reading,
    refetch,
    editor,
    readOnly: () => readOnlyNow.current !== null,
    rendered: () => hidden.current,
    firstConflict: toFirstConflict,
  });
  // Before the paint, so the Source view never shows a frame at the old place.
  useLayoutEffect(() => {
    if (showRendered || !waiting.current) return;
    waiting.current = false;
    sourceShown({ editor: editor.current, onBody: document.activeElement !== null && document.activeElement === root.current?.closest(".preview__body") });
  }, [showRendered]);
  const hintId = useId();
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  // A shebang tells the type of a file whose name does not: read from the text the tab opened with.
  const firstLine = useRef<string | undefined>(undefined);
  if (firstLine.current === undefined && text) firstLine.current = text.text.split(/\r\n|\r|\n/, 1)[0] ?? "";
  // The editor keeps the choice while the tab is loaded.
  const wrap = wrapAtStart(kind, entry.path, firstLine.current);
  const appChord = useCallback((e: KeyboardEvent) => findCommand(e, os) !== undefined, [os]);

  // The rendered view gets the editor's text 150 ms after the last key, and the file's own text until the document is open.
  const [rendered, setRendered] = useState(() => lf(text?.text ?? ""));
  const shownOnce = useRef(false);
  useEffect(() => {
    if (!paged || !ready) return;
    // The first text of the open document shows at once; later ones wait for the typing to stop.
    const first = !shownOnce.current;
    shownOnce.current = true;
    if (rendered === doc.text) return;
    if (first) {
      setRendered(doc.text);
      return;
    }
    const timer = setTimeout(() => setRendered(doc.text), RENDER_MS);
    return () => clearTimeout(timer);
  }, [paged, ready, doc.text, rendered]);

  const latest = useRef({ doc, readOnly });
  latest.current = { doc, readOnly };
  const say = useRef(announce);
  say.current = announce;
  useImperativeHandle(
    commands,
    () => ({
      save,
      undoSave() {
        const { doc: d } = latest.current;
        if (d.lastSave === null || d.saving) return false;
        dispatch({ type: "unsave" });
        return true;
      },
      canUndoSave: () => latest.current.doc.lastSave !== null && !latest.current.doc.saving,
      discard,
      goToLine: () => editor.current?.goToLine(),
      toggleWrap: () => editor.current?.toggleWrap(),
      nextConflict: () => conflictJump({ rendered: hidden.current, editor: editor.current, announce: say.current }, "next"),
      previousConflict: () => conflictJump({ rendered: hidden.current, editor: editor.current, announce: say.current }, "previous"),
      dirty: () => latest.current.doc.dirty,
      focus: () => editor.current?.focus(),
    }),
    [dispatch, save, discard],
  );

  // Said once when the document opens, clean or not: the window may show a dot for a draft that turned out to match the disk.
  const saidDirty = useRef<boolean | null>(null);
  useEffect(() => {
    const news = dirtyNews(saidDirty.current, ready, doc.dirty);
    if (news === null) return;
    saidDirty.current = news;
    onDirty(entry.path, news);
  }, [ready, doc.dirty, entry.path, onDirty]);

  // A tab that is no longer on screen leaves its marks and its offer to undo behind.
  useEffect(() => {
    if (!active) dispatch({ type: "left" });
  }, [active, dispatch]);

  // The bar takes no focus, so a screen reader hears of it through the announcer.
  const conflict = doc.conflict?.hash ?? null;
  const told = useRef<string | null>(null);
  useEffect(() => {
    if (conflict !== null && conflict !== told.current) announce(`${name} changed on disk.`);
    told.current = conflict;
  }, [conflict, name, announce]);

  // A line the pane was opened at is scrolled to once for each entry; a refetch keeps the reader's place.
  const line = entry.line;
  useEffect(() => {
    if (ready && line !== null) editor.current?.reveal(line);
  }, [entry, ready]);

  // Each of the three buttons goes away once it is pressed, so the keyboard moves to the text first.
  const pressed = (event: DocEvent) => {
    keyboardTo({ rendered: paged && entry.view === "rendered", editor: editor.current, body: root.current?.closest<HTMLElement>(".preview__body") ?? null });
    dispatch(event);
  };

  return (
    <TextTabView
      entry={entry}
      name={name}
      kind={kind}
      doc={doc}
      ready={ready}
      gone={file.phase === "deleted"}
      readOnly={readOnly}
      encoding={text?.encoding ?? "utf-8"}
      eol={text?.eol ?? "lf"}
      cursor={cursor}
      wrap={wrap}
      hintId={hintId}
      rendered={rendered}
      firstLine={firstLine.current}
      editor={editor}
      root={root}
      onEdit={(t) => dispatch({ type: "edited", text: t })}
      onCursor={(l, c) => setCursor((p) => (p.line === l && p.column === c ? p : { line: l, column: c }))}
      appChord={appChord}
      onMerge={() => pressed({ type: "merge" })}
      onConflict={(at, of) => announce(of > 0 ? `Conflict ${at} of ${of}.` : `${name} has no conflicts.`)}
      onKeep={() => pressed({ type: "keepMine" })}
      onTake={() => pressed({ type: "takeDisk" })}
      onUndo={() => pressed({ type: "unsave" })}
      onLink={onLink}
      onKey={onKey}
    />
  );
}
