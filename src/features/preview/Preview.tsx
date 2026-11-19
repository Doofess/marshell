import { useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode, type Ref } from "react";
import { createPortal } from "react-dom";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { openUrl } from "@tauri-apps/plugin-opener";
import { cropImage, cropPdfPage } from "../regions/crop";
import { placePopover } from "../regions/placement";
import { RegionComment, type RegionTarget } from "../regions/RegionComment";
import { RegionLayer } from "../regions/RegionLayer";
import { getDraft } from "../editor/editorApi";
import { isSaveKey, TextTab, type TextTabCommands } from "../editor/TextTab";
import { canMarkRegion, fromDrag, initialRegion, regionLabel, regionStillValid, type Region } from "../regions/region";
import type { Os } from "../../lib/keymap";
import { ImageView } from "./ImageView";
import { kindOf, splitPath } from "./kinds";
import type { LinkTarget } from "./links";
import { PdfView } from "./pdf/PdfView";
import { useOpenPageInPane, type OpenInPane } from "../pane/PaneContext";
import type { PaneEntry, View } from "../pane/paneState";
import { usePreviewFile, type FileState } from "./usePreviewFile";
import { followWebLink } from "./webLink";
import { WebLinkBar } from "./WebLinkBar";
import "./Preview.css";

/**
 * Whether the body takes the keyboard when the tab's entry changes. It does when the tab goes somewhere, but not for the Source
 * view a merge showed (`quiet`): that answer can come while the person is in the terminal, and the text tab moves a keyboard
 * that was on the body on into the editor itself (6q-4b decision 7.2). A quiet switch that never came keeps nothing quiet.
 */
export function bodyTakesFocus(was: PaneEntry, now: PaneEntry, quiet: boolean): boolean {
  const viewOnly = was.path === now.path && was.line === now.line && was.col === now.col && was.view !== now.view;
  return !(quiet && viewOnly);
}

/** A size for a sentence: "300 KB", "1.5 MB". */
export function formatSize(bytes: number): string {
  const one = (v: number) => String(Math.round(v * 10) / 10);
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${one(bytes / 1024)} KB`;
  return `${one(bytes / (1024 * 1024))} MB`;
}

const arrow = (d: string) => (
  <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

const viewIcon = (...d: string[]) => (
  <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    {d.map((p) => (
      <path key={p} d={p} />
    ))}
  </svg>
);

/** The Rendered / Source toggle's icons: an eye for the rendered view, `</>` for the source. */
const VIEW_ICONS = {
  rendered: viewIcon("M1.5 7S3.5 3 7 3s5.5 4 5.5 4-2 4-5.5 4S1.5 7 1.5 7Z", "M8.75 7a1.75 1.75 0 1 1-3.5 0 1.75 1.75 0 0 1 3.5 0Z"),
  source: viewIcon("M4.5 4 1.5 7l3 3", "M9.5 4l3 3-3 3", "M8 2.5 6 11.5"),
} as const;

export type PreviewPaneProps = {
  entry: PaneEntry;
  file: FileState;
  /** A refetch just happened: the header says "Updated" for a moment. */
  updated: boolean;
  os: Os;
  canBack: boolean;
  canForward: boolean;
  onBack: () => void;
  onForward: () => void;
  /** A local link in a document: opens that file here, pushing history. */
  onOpen: OpenInPane;
  /** The Rendered / Source toggle of a Markdown or HTML file. */
  onView: (view: View) => void;
  /** The tab is the selected one; a kept tab that isn't takes no focus and checks its file no more. */
  active: boolean;
  /** Escape: focus goes back to the session's terminal and the pane stays open. */
  onEscape: () => void;
  /** Says a line through the window's announcer: a save, a file that changed under edits. */
  announce: (text: string) => void;
  /** Reads the file again now, whatever its stat says. */
  refetch: () => void;
  /** Where a marked region goes: the focused session's name and how it gets there, or null with no session. */
  regionTarget: RegionTarget | null;
  /** Hands over a marked region's crop and comment. Resolves when it is placed; rejects with the reason it was not. */
  onRegion: (r: { png: Blob; label: string; comment: string }) => Promise<void>;
  /** A text tab's file gained or lost unsaved edits. The window keeps the dot on the tab strip by it. */
  onDirty?: (path: string, dirty: boolean) => void;
  /** Unsaved edits are kept for this file, which was already gone when the tab opened: the editor opens on them. */
  keptDraft?: boolean;
  /** The palette's "Select region" starts a region through this. */
  ref?: Ref<PreviewHandle>;
};

/** What a parent holds of a preview tab. */
export type PreviewHandle = {
  /** Starts marking a region. False when the file has no page to mark (a PDF still loading, text, an error). */
  selectRegion(): boolean;
  /** The text tab's commands, or null while the file is not an editor that has loaded. */
  text(): TextTabCommands | null;
};

/** What the pane keeps while a region is being marked. */
type Select = { region: Region; step: "box" | "comment"; busy: boolean; error: string | null };

/** What a page-bearing view (a PDF or an image) needs to let a region be marked on it. */
type RegionHooks = {
  overlay: (page: number) => ReactNode;
  onCurrentPage: (page: number) => void;
  onDocument: (doc: PDFDocumentProxy | null) => void;
};

/** The keys the pane answers to; a native event from inside the HTML frame has the same shape as React's. */
type Key = { key: string; code: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean; preventDefault: () => void };

/** A link that goes somewhere outside the document's own scroll: a file, or a web address. */
type Outbound = Exclude<LinkTarget, { kind: "anchor" } | { kind: "none" }>;

type BodyProps = {
  entry: PaneEntry;
  file: FileState;
  active: boolean;
  os: Os;
  announce: (text: string) => void;
  refetch: () => void;
  /** What a text tab hands the pane to call. */
  textCommands: Ref<TextTabCommands>;
  onDirty: (path: string, dirty: boolean) => void;
  onImageSize: (url: string, w: number, h: number) => void;
  onPages: (count: number) => void;
  onLink: (link: Outbound) => void;
  onKey: (e: Key) => void;
  region: RegionHooks;
  keptDraft: boolean;
  /** Shows a Markdown or HTML tab's Source view, after a merge that left conflicts; it doesn't move the keyboard. */
  onSource: () => void;
};

const noDirty = () => {};

function Body({ entry, file, active, os, announce, refetch, textCommands, onDirty, onImageSize, onPages, onLink, onKey, region, keptDraft, onSource }: BodyProps) {
  const { name } = splitPath(entry.path);
  // A text file, and a deleted one that was text, are the same tab: the editor stays mounted when the file goes and comes back.
  const textTab = (f: Extract<FileState, { phase: "text" | "deleted" }>) => (
    <TextTab key={entry.path} entry={entry} file={f} kind={kindOf(entry.path)} active={active} os={os} onDirty={onDirty} commands={textCommands} announce={announce} refetch={refetch} onLink={onLink} onKey={onKey} onSource={onSource} />
  );
  switch (file.phase) {
    case "loading":
      return <p className="preview__message">Loading…</p>;
    case "text":
      return textTab(file);
    case "image":
      return <ImageView url={file.url} name={name} onSize={(w, h) => onImageSize(file.url, w, h)} overlay={region.overlay(1)} />;
    case "tooLarge":
      return <p className="preview__message">{`This file is too large to preview. It is ${formatSize(file.size)} and the limit is ${formatSize(file.limit)}.`}</p>;
    case "binary":
      return <p className="preview__message">This file isn't text.</p>;
    case "pdf":
      return <PdfView bytes={file.bytes} version={file.version} name={name} onPages={onPages} onCurrentPage={region.onCurrentPage} onDocument={region.onDocument} renderOverlay={region.overlay} />;
    case "deleted":
      // With something that was shown, the content stays and the notice sits above it (see PreviewPane); this is the plain case.
      // A file that was gone before the tab ever showed it has only its draft: the editor opens on that, so the edits can be read
      // and saved again.
      if (file.last?.phase === "text" || (file.last === undefined && keptDraft)) return textTab(file);
      return file.last ? <Body entry={entry} file={file.last} active={active} os={os} announce={announce} refetch={refetch} textCommands={textCommands} onDirty={onDirty} onImageSize={onImageSize} onPages={onPages} onLink={onLink} onKey={onKey} region={region} keptDraft={keptDraft} onSource={onSource} /> : <p className="preview__message">This file was deleted.</p>;
    case "error":
      return <p className="preview__message">This file can't be previewed.</p>;
  }
}

/** The pane for one file: header, then the scrolling body. Everything it shows comes in as props, so it renders the same on any state. */
export function PreviewPane({ entry, file, updated, os, canBack, canForward, onBack, onForward, onOpen, onView, active, onEscape, announce, refetch, regionTarget, onRegion, onDirty, keptDraft = false, ref }: PreviewPaneProps) {
  const { name, folder } = splitPath(entry.path);
  const body = useRef<HTMLDivElement>(null);
  // What a text tab lets the pane call: Ctrl+S saves through it. Null while no text tab is mounted.
  const textCommands = useRef<TextTabCommands>(null);
  const noteDirty = onDirty ?? noDirty;
  // Focus goes into the scroll body when the tab goes somewhere (a link, Back, Forward, another line), so the arrow keys and
  // Page Up and Down read it at once. Mounting takes no focus, and neither does being selected: a session switch and the tab
  // strip's own keys mount and select tabs, and focus must stay where the person has it. Whoever opens a file moves focus.
  const activeRef = useRef(active);
  activeRef.current = active;
  const shownEntry = useRef(entry);
  // The text tab asked for its Source view after a merge; the next entry change says whether it came.
  const quietView = useRef(false);
  const toSource = () => {
    quietView.current = true;
    onView("source");
  };
  useEffect(() => {
    if (shownEntry.current === entry) return;
    const was = shownEntry.current;
    shownEntry.current = entry;
    const quiet = quietView.current;
    quietView.current = false;
    if (activeRef.current && bodyTakesFocus(was, entry, quiet)) body.current?.focus({ preventScroll: true });
  }, [entry]);
  // "Updated" is spoken only to someone reading in the pane; a person typing in the terminal is not interrupted by it.
  const [inside, setInside] = useState(false);
  const [size, setSize] = useState<{ url: string; w: number; h: number } | null>(null);
  // A deleted file keeps showing what it last was; the notice below says so.
  const shown = file.phase === "deleted" && file.last ? file.last : file;
  // A text tab shows its own notice, with the way back.
  const gone = file.phase === "deleted" && file.last !== undefined && file.last.phase !== "text";
  const [pages, setPages] = useState<{ path: string; count: number } | null>(null);
  const dims =
    shown.phase === "image" && size?.url === shown.url
      ? `${size.w} × ${size.h}`
      : shown.phase === "pdf" && pages?.path === entry.path
        ? `${pages.count} ${pages.count === 1 ? "page" : "pages"}`
        : null;

  // A web link waits here for the person's answer; moving to another file drops the question.
  const [web, setWeb] = useState<string | null>(null);
  useEffect(() => setWeb(null), [entry]);
  const answer = (open: boolean) => {
    if (open && web) void openUrl(web).catch(() => {});
    setWeb(null);
    body.current?.focus({ preventScroll: true });
  };
  // Where the side pane has browser tabs a web link opens one at once, with no question; elsewhere the question stays.
  const openPage = useOpenPageInPane();
  const follow = (link: Outbound) => {
    if (link.kind === "web") {
      if (followWebLink(link.url, openPage) === "ask") setWeb(link.url);
    } else onOpen(link.line === undefined ? { path: link.path } : { path: link.path, line: link.line });
  };

  // Marking a region: only an image or a PDF has pages to mark.
  const [pdf, setPdf] = useState<PDFDocumentProxy | null>(null);
  const canMark = canMarkRegion(shown.phase, pdf !== null);
  const [select, setSelect] = useState<Select | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [focusBox, setFocusBox] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  const drag = useRef<{ page: number; from: { x: number; y: number } } | null>(null);
  const pageCount = shown.phase === "pdf" ? (pages?.path === entry.path ? pages.count : 0) : 1;

  // Another file, or a file that no longer has the box's page, ends it.
  useEffect(() => setSelect(null), [entry]);
  useEffect(() => {
    if (select && pageCount > 0 && !regionStillValid(select.region, pageCount)) setSelect(null);
  }, [select, pageCount]);
  useEffect(() => {
    if (!canMark) setSelect(null);
  }, [canMark]);
  // The box takes focus when it is shown, and again when the comment hands it back.
  const step = select?.step;
  const marking = select !== null;
  useEffect(() => {
    if (marking && step === "box") box.current?.focus({ preventScroll: true });
  }, [marking, step, focusBox]);

  const endSelect = () => {
    setSelect(null);
    body.current?.focus({ preventScroll: true });
  };
  const startSelect = () => {
    if (!canMark) return;
    // The part of the current page that is on screen, as fractions of it.
    let visible = { top: 0, bottom: 1 };
    const sc = body.current;
    const el = sc?.querySelector<HTMLElement>(`[data-page="${currentPage}"]`);
    if (sc && el) {
      const b = sc.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      if (r.height > 0) visible = { top: Math.max(0, (b.top - r.top) / r.height), bottom: Math.min(1, (b.bottom - r.top) / r.height) };
      if (visible.bottom <= visible.top) visible = { top: 0, bottom: 1 };
    }
    setSelect({ region: initialRegion(shown.phase === "pdf" ? currentPage : 1, visible), step: "box", busy: false, error: null });
    setFocusBox((n) => n + 1);
  };
  useImperativeHandle(ref, () => ({
    selectRegion() {
      if (!canMark) return false;
      if (!select) startSelect();
      return true;
    },
    text: () => textCommands.current,
  }));
  const add = async (comment: string) => {
    if (!select) return;
    const region = select.region;
    setSelect({ ...select, busy: true, error: null });
    try {
      let png: Blob;
      if (shown.phase === "pdf") {
        if (!pdf) throw new Error("The PDF is not loaded.");
        png = await cropPdfPage(pdf, region);
      } else if (shown.phase === "image") png = await cropImage(shown.url, region);
      else throw new Error("There is nothing to crop.");
      await onRegion({ png, label: regionLabel(name, shown.phase === "pdf" ? region.page : null), comment });
      setSelect(null);
    } catch (e) {
      setSelect((s) => (s ? { ...s, busy: false, error: e instanceof Error ? e.message : String(e) } : s));
    }
  };

  const capture = (page: number) => ({
    onPointerDown: (e: ReactPointerEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      const r = e.currentTarget.getBoundingClientRect();
      drag.current = { page, from: { x: e.clientX - r.left, y: e.clientY - r.top } };
      e.currentTarget.setPointerCapture(e.pointerId);
      e.preventDefault();
    },
    onPointerMove: (e: ReactPointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      if (!d) return;
      const r = e.currentTarget.getBoundingClientRect();
      const next = fromDrag(d.page, d.from, { x: e.clientX - r.left, y: e.clientY - r.top }, { w: r.width, h: r.height });
      if (next) setSelect((s) => (s ? { ...s, region: next, step: "box" } : s));
    },
    onPointerUp: (e: ReactPointerEvent<HTMLDivElement>) => {
      if (!drag.current) return;
      drag.current = null;
      if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
      setFocusBox((n) => n + 1);
    },
    onDoubleClick: () => setSelect((s) => (s ? { ...s, step: "comment" } : s)),
  });
  const regionHooks: RegionHooks = {
    overlay: (page) =>
      select && (shown.phase === "image" || shown.phase === "pdf") ? (
        <>
          <div className="region-capture" {...capture(page)} />
          {select.region.page === page && (
            <RegionLayer
              ref={box}
              region={select.region}
              onChange={(region) => setSelect((s) => (s ? { ...s, region } : s))}
              onConfirm={() => setSelect((s) => (s ? { ...s, step: "comment", error: null } : s))}
              onCancel={endSelect}
            />
          )}
        </>
      ) : null,
    onCurrentPage: setCurrentPage,
    onDocument: setPdf,
  };

  const keys = (e: Key) => {
    const t = (e as { target?: unknown }).target as HTMLElement | null | undefined;
    const inField = !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable === true);
    // Escape belongs to the editor while it has the keyboard: it closes the find bar, and Esc then Tab leaves.
    const inEditor = !!t && t.closest?.(".editor") != null;
    if (isSaveKey(e, os) && textCommands.current) {
      // Always taken, so the webview's own save never opens, whether or not there is anything to save.
      e.preventDefault();
      void textCommands.current.save();
    } else if ((e.key === "r" || e.key === "R") && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey && canMark && !inField) {
      e.preventDefault();
      if (select) endSelect();
      else startSelect();
    } else if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
      if (inEditor) return;
      e.preventDefault();
      onEscape();
    } else if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && e.key === "ArrowLeft") {
      e.preventDefault();
      if (canBack) onBack();
    } else if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && e.key === "ArrowRight") {
      e.preventDefault();
      if (canForward) onForward();
    }
  };

  // Markdown and HTML can be read rendered or as source; the toggle is there once there is text to show.
  const kind = kindOf(entry.path);
  const toggle = shown.phase === "text" && (kind === "markdown" || kind === "html");
  const rendered = toggle && entry.view === "rendered";
  // A text tab says a change from disk through its marks and the announcer, so the header's note stays out of it.
  const showUpdated = updated && shown.phase !== "text";

  const alt = os === "mac" ? "⌥" : "Alt+";
  return (
    <section
      className="preview"
      data-kind={kindOf(entry.path)}
      onKeyDown={keys}
      onFocus={() => setInside(true)}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setInside(false);
      }}
    >
      <header className="preview__header">
        <button type="button" className="preview__button" aria-label="Back" title={`Back (${alt}${os === "mac" ? "←" : "Left"})`} disabled={!canBack} onClick={onBack}>
          {arrow("M7.5 2.5 4 6l3.5 3.5")}
        </button>
        <button type="button" className="preview__button" aria-label="Forward" title={`Forward (${alt}${os === "mac" ? "→" : "Right"})`} disabled={!canForward} onClick={onForward}>
          {arrow("M4.5 2.5 8 6 4.5 9.5")}
        </button>
        <span className="preview__name">{name}</span>
        {folder && (
          <span className="preview__folder" title={folder}>
            {folder}
          </span>
        )}
        {dims && <span className="preview__meta">{dims}</span>}
        <span className="preview__updated" data-show={showUpdated ? "true" : undefined} aria-live={inside ? "polite" : "off"}>
          {showUpdated ? "Updated" : ""}
        </span>
        <span className="preview__spacer" />
        {toggle && (
          <div className="preview__views" role="group" aria-label="View">
            {(["rendered", "source"] as const).map((v) => {
              const label = v === "rendered" ? "Rendered" : "Source";
              return (
                <button key={v} type="button" className="preview__view" aria-label={label} title={label} aria-pressed={entry.view === v} onClick={() => onView(v)}>
                  {VIEW_ICONS[v]}
                </button>
              );
            })}
          </div>
        )}
        {canMark && (
          <button type="button" className="preview__button" aria-label="Select region" aria-pressed={select !== null} title="Select region (R)" onClick={() => (select ? endSelect() : startSelect())}>
            <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeDasharray="2 1.5">
              <rect x="1.5" y="2.5" width="9" height="7" />
            </svg>
          </button>
        )}
      </header>
      {web && <WebLinkBar url={web} onOpen={() => answer(true)} onCancel={() => answer(false)} />}
      <div ref={body} className="preview__body" tabIndex={0} role="group" aria-label={`${name}, contents`} data-view={rendered ? kind : undefined} data-selecting={select ? "true" : undefined}>
        {gone && (
          <div className="preview__notice" role="status" aria-live={inside ? "polite" : "off"}>
            This file was deleted.
          </div>
        )}
        <Body entry={entry} file={file} active={active} os={os} announce={announce} refetch={refetch} textCommands={textCommands} onDirty={noteDirty} onImageSize={(url, w, h) => setSize({ url, w, h })} onPages={(count) => setPages({ path: entry.path, count })} onLink={follow} onKey={keys} region={regionHooks} keptDraft={keptDraft} onSource={toSource} />
      </div>
      {select?.step === "comment" && body.current
        ? createPortal(
            <RegionPopover boxEl={box.current} body={body.current} region={select.region}>
              <RegionComment target={regionTarget} busy={select.busy} error={select.error} onAdd={(c) => void add(c)} onCancel={() => setSelect((s) => (s ? { ...s, step: "box", error: null } : s))} />
            </RegionPopover>,
            body.current,
          )
        : null}
    </section>
  );
}

/** The comment box beside the marked region, placed below or above it and kept inside the pane's body. It follows the body's scroll and any resize. */
function RegionPopover({ boxEl, body, region, children }: { boxEl: HTMLElement | null; body: HTMLElement; region: Region; children: ReactNode }) {
  const holder = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const place = () => {
      const pop = holder.current;
      if (!pop || !boxEl) return;
      const b = boxEl.getBoundingClientRect();
      const v = body.getBoundingClientRect();
      const p = placePopover(
        { left: b.left, top: b.top, width: b.width, height: b.height },
        { w: pop.offsetWidth, h: pop.offsetHeight },
        { left: v.left, top: v.top, width: v.width, height: v.height },
      );
      setAt((prev) => (prev && prev.left === p.left && prev.top === p.top ? prev : { left: p.left, top: p.top }));
    };
    place();
    body.addEventListener("scroll", place, { passive: true });
    window.addEventListener("resize", place);
    // The pane, the page under the box and the popover itself can all change size without the window moving (a split dragged, a zoom, an error line appearing).
    const watch = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(place);
    for (const el of [body, boxEl, boxEl?.parentElement, holder.current]) if (el) watch?.observe(el);
    return () => {
      body.removeEventListener("scroll", place);
      window.removeEventListener("resize", place);
      watch?.disconnect();
    };
  }, [boxEl, body, region]);
  // The popover is hidden until it is placed, and a hidden input cannot take focus; so focus moves in once it shows.
  const focused = useRef(false);
  useEffect(() => {
    if (!at || focused.current) return;
    focused.current = true;
    holder.current?.querySelector<HTMLInputElement>("input")?.focus({ preventScroll: true });
  }, [at]);
  return (
    <div ref={holder} className="region-popover" style={at ? { left: at.left, top: at.top } : { left: 0, top: 0, visibility: "hidden" }}>
      {children}
    </div>
  );
}

export type PreviewProps = Omit<PreviewPaneProps, "file" | "updated" | "refetch" | "keptDraft">;

/**
 * Whether unsaved edits are kept for a file that was already gone when its tab opened. Asked once the file is known to be gone,
 * and the answer stays for the path: the editor that opens on the draft must not go away when the draft is saved or emptied.
 */
function useKeptDraft(path: string, lost: boolean): boolean {
  const [kept, setKept] = useState<string | null>(null);
  useEffect(() => {
    if (!lost) return;
    let on = true;
    void getDraft(path).then((draft) => {
      if (on && draft !== null) setKept(path);
    });
    return () => {
      on = false;
    };
  }, [path, lost]);
  return kept === path;
}

/** The preview pane for the current entry. It loads the file, keeps it current while it is open, and stops when it closes. */
export function Preview(props: PreviewProps) {
  const { file, updated, refetch } = usePreviewFile(props.entry.path, { paused: !props.active });
  const keptDraft = useKeptDraft(props.entry.path, file.phase === "deleted" && file.last === undefined);
  return <PreviewPane {...props} file={file} updated={updated} refetch={refetch} keptDraft={keptDraft} />;
}
