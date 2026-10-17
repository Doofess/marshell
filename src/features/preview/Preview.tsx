import { useEffect, useRef, useState, type CSSProperties } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ITheme } from "@xterm/xterm";
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
};

function Body({ entry, file, onImageSize, onPages, onLink, onKey }: BodyProps) {
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
      return <ImageView url={file.url} name={name} onSize={(w, h) => onImageSize(file.url, w, h)} />;
    case "tooLarge":
      return <p className="preview__message">{`This file is too large to preview. It is ${formatSize(file.size)} and the limit is ${formatSize(file.limit)}.`}</p>;
    case "binary":
      return <p className="preview__message">This file isn't text.</p>;
    case "pdf":
      return <PdfView bytes={file.bytes} version={file.version} name={name} onPages={onPages} />;
    case "deleted":
      // With something that was shown, the content stays and the notice sits above it (see PreviewPane); this is the plain case.
      return file.last ? <Body entry={entry} file={file.last} onImageSize={onImageSize} onPages={onPages} onLink={onLink} onKey={onKey} /> : <p className="preview__message">This file was deleted.</p>;
    case "error":
      return <p className="preview__message">This file can't be previewed.</p>;
  }
}

/** The pane for one file: header, then the scrolling body. Everything it shows comes in as props, so it renders the same on any state. */
export function PreviewPane({ entry, file, updated, theme, os, canBack, canForward, onBack, onForward, onOpen, onView, onClose, onEscape }: PreviewPaneProps) {
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

  const keys = (e: Key) => {
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
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
        <button type="button" className="preview__button" aria-label="Close preview" title={`Close preview (${shortcutLabel(commandById("preview").binding!, os)})`} onClick={onClose}>
          {arrow("M3 3l6 6M9 3 3 9")}
        </button>
      </header>
      {web && <WebLinkBar url={web} onOpen={() => answer(true)} onCancel={() => answer(false)} />}
      <div ref={body} className="preview__body" tabIndex={0} role="group" aria-label={`${name}, contents`} data-view={rendered ? kind : undefined}>
        {gone && (
          <div className="preview__notice" role="status" aria-live={inside ? "polite" : "off"}>
            This file was deleted.
          </div>
        )}
        <Body entry={entry} file={file} onImageSize={(url, w, h) => setSize({ url, w, h })} onPages={(count) => setPages({ path: entry.path, count })} onLink={follow} onKey={keys} />
      </div>
    </section>
  );
}

export type PreviewProps = Omit<PreviewPaneProps, "file" | "updated">;

/** The preview pane for the current entry. It loads the file, keeps it current while it is open, and stops when it closes. */
export function Preview(props: PreviewProps) {
  const { file, updated } = usePreviewFile(props.entry.path);
  return <PreviewPane {...props} file={file} updated={updated} />;
}
