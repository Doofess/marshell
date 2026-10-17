import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ITheme } from "@xterm/xterm";
import { cropImage, cropPdfPage } from "../regions/crop";
import { placePopover } from "../regions/placement";
import { RegionComment, type RegionTarget } from "../regions/RegionComment";
import { RegionLayer } from "../regions/RegionLayer";
import { canMarkRegion, fromDrag, initialRegion, regionLabel, regionStillValid, type Region } from "../regions/region";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { CodeView } from "./CodeView";
import { themeVariables } from "./highlight";
import { HtmlView } from "./HtmlView";
import { ImageView } from "./ImageView";
import { kindOf, languageOf, splitPath } from "./kinds";
import type { LinkTarget } from "./links";
import { MarkdownView } from "./MarkdownView";
import { PdfView } from "./pdf/PdfView";
import type { OpenPreview } from "./PreviewContext";
import type { PreviewEntry } from "./previewState";
import { usePreviewFile, type FileState } from "./usePreviewFile";
import { WebLinkBar } from "./WebLinkBar";
import "./Preview.css";

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

export type PreviewPaneProps = {
  entry: PreviewEntry;
  file: FileState;
  /** A refetch just happened: the header says "Updated" for a moment. */
  updated: boolean;
  /** The terminal theme beside the pane; the code takes its colours from it. */
  theme: ITheme;
  os: Os;
  canBack: boolean;
  canForward: boolean;
  onBack: () => void;
  onForward: () => void;
  /** A local link in a document: opens that file here, pushing history. */
  onOpen: OpenPreview;
  /** The Rendered / Source toggle of a Markdown or HTML file. */
  onView: (view: PreviewEntry["view"]) => void;
  onClose: () => void;
  /** Escape: focus goes back to the session's terminal and the pane stays open. */
  onEscape: () => void;
  /** Where a marked region goes: the focused session's name and how it gets there, or null with no session. */
  regionTarget: RegionTarget | null;
  /** Hands over a marked region's crop and comment. Resolves when it is placed; rejects with the reason it was not. */
  onRegion: (r: { png: Blob; label: string; comment: string }) => Promise<void>;
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
type Key = { key: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean; preventDefault: () => void };

/** A link that goes somewhere outside the document's own scroll: a file, or a web address. */
type Outbound = Exclude<LinkTarget, { kind: "anchor" } | { kind: "none" }>;

type BodyProps = {
  entry: PreviewEntry;
  file: FileState;
  onImageSize: (url: string, w: number, h: number) => void;
  onPages: (count: number) => void;
  onLink: (link: Outbound) => void;
  onKey: (e: Key) => void;
  region: RegionHooks;
};

function Body({ entry, file, onImageSize, onPages, onLink, onKey, region }: BodyProps) {
  const { name } = splitPath(entry.path);
  switch (file.phase) {
    case "loading":
      return <p className="preview__message">Loading…</p>;
    case "text": {
      const kind = kindOf(entry.path);
      if (entry.view === "rendered" && kind === "markdown") return <MarkdownView text={file.text} docPath={entry.path} onLink={onLink} />;
      if (entry.view === "rendered" && kind === "html") return <HtmlView text={file.text} name={name} docPath={entry.path} onLink={onLink} onKey={onKey} />;
      return <CodeView text={file.text} language={languageOf(entry.path)} target={entry} />;
    }
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
      return file.last ? <Body entry={entry} file={file.last} onImageSize={onImageSize} onPages={onPages} onLink={onLink} onKey={onKey} region={region} /> : <p className="preview__message">This file was deleted.</p>;
    case "error":
      return <p className="preview__message">This file can't be previewed.</p>;
  }
}

/** The pane for one file: header, then the scrolling body. Everything it shows comes in as props, so it renders the same on any state. */
export function PreviewPane({ entry, file, updated, theme, os, canBack, canForward, onBack, onForward, onOpen, onView, onClose, onEscape, regionTarget, onRegion }: PreviewPaneProps) {
  const { name, folder } = splitPath(entry.path);
  const body = useRef<HTMLDivElement>(null);
  // Focus goes into the scroll body when a file is opened, so the arrow keys and Page Up and Down read it at once.
  useEffect(() => {
    body.current?.focus({ preventScroll: true });
  }, [entry]);
  // "Updated" is spoken only to someone reading in the pane; a person typing in the terminal is not interrupted by it.
  const [inside, setInside] = useState(false);
  const [size, setSize] = useState<{ url: string; w: number; h: number } | null>(null);
  // A deleted file keeps showing what it last was; the notice below says so.
  const shown = file.phase === "deleted" && file.last ? file.last : file;
  const gone = file.phase === "deleted" && file.last !== undefined;
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
  const follow = (link: Outbound) => {
    if (link.kind === "web") setWeb(link.url);
    else onOpen(link.line === undefined ? { path: link.path } : { path: link.path, line: link.line });
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
    if ((e.key === "r" || e.key === "R") && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey && canMark && !inField) {
      e.preventDefault();
      if (select) endSelect();
      else startSelect();
    } else if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
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

  const alt = os === "mac" ? "⌥" : "Alt+";
  return (
    <section
      className="preview"
      aria-label={`Preview, ${name}`}
      data-kind={kindOf(entry.path)}
      style={themeVariables(theme) as CSSProperties}
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
        <span className="preview__updated" data-show={updated ? "true" : undefined} aria-live={inside ? "polite" : "off"}>
          {updated ? "Updated" : ""}
        </span>
        <span className="preview__spacer" />
        {toggle && (
          <div className="preview__views" role="group" aria-label="View">
            {(["rendered", "source"] as const).map((v) => (
              <button key={v} type="button" className="preview__view" aria-pressed={entry.view === v} onClick={() => onView(v)}>
                {v === "rendered" ? "Rendered" : "Source"}
              </button>
            ))}
          </div>
        )}
        {canMark && (
          <button type="button" className="preview__button" aria-label="Select region" aria-pressed={select !== null} title="Select region (R)" onClick={() => (select ? endSelect() : startSelect())}>
            <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeDasharray="2 1.5">
              <rect x="1.5" y="2.5" width="9" height="7" />
            </svg>
          </button>
        )}
        <button type="button" className="preview__button" aria-label="Close preview" title={`Close preview (${shortcutLabel(commandById("preview").binding!, os)})`} onClick={onClose}>
          {arrow("M3 3l6 6M9 3 3 9")}
        </button>
      </header>
      {web && <WebLinkBar url={web} onOpen={() => answer(true)} onCancel={() => answer(false)} />}
      <div ref={body} className="preview__body" tabIndex={0} role="group" aria-label={`${name}, contents`} data-view={rendered ? kind : undefined} data-selecting={select ? "true" : undefined}>
        {gone && (
          <div className="preview__notice" role="status" aria-live={inside ? "polite" : "off"}>
            This file was deleted.
          </div>
        )}
        <Body entry={entry} file={file} onImageSize={(url, w, h) => setSize({ url, w, h })} onPages={(count) => setPages({ path: entry.path, count })} onLink={follow} onKey={keys} region={regionHooks} />
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

export type PreviewProps = Omit<PreviewPaneProps, "file" | "updated">;

/** The preview pane for the current entry. It loads the file, keeps it current while it is open, and stops when it closes. */
export function Preview(props: PreviewProps) {
  const { file, updated } = usePreviewFile(props.entry.path);
  return <PreviewPane {...props} file={file} updated={updated} />;
}
