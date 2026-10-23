import { useEffect, useId, useRef, useState } from "react";
import { exportFileName, exportTitle, type ExportFormat } from "../../lib/exportName";
import { useDialogFocus } from "../../lib/useDialogFocus";
import "./ExportDialog.css";

const formatName = (format: ExportFormat) => (format === "markdown" ? "markdown" : format === "json" ? "JSON" : "asciicast");

/** The core's words when an asciicast is asked for and the conversation has no recording. */
export const NO_RECORDING_COPY = "Marshell has no recording of this conversation.";

/** How many lines of a recording the preview shows; Save writes all of it. */
export const PREVIEW_LINES = 200;

/** The first `max` lines of `text`, for a preview of a file that can be very large. */
export function firstLines(text: string, max = PREVIEW_LINES): string {
  let end = -1;
  for (let n = 0; n < max; n++) {
    end = text.indexOf("\n", end + 1);
    if (end === -1) return text;
  }
  return text.slice(0, end);
}

const isNotFound = (e: unknown) => typeof e === "object" && e !== null && (e as { status?: unknown }).status === 404;

/** Saves an export under the name `exportFileName` gives, and says where it went. A cancelled Save dialog says nothing. */
export async function saveExport(p: {
  text: string;
  title: string;
  format: ExportFormat;
  /** The text was exported with secrets and personal paths hidden, so the file is named from its title, not the raw one. */
  redact?: boolean;
  onSave: (text: string, name: string) => Promise<string | null>;
  announce: (text: string) => void;
  now?: Date;
}): Promise<void> {
  const path = await p.onSave(p.text, exportFileName(exportTitle(p.text, p.format, p.title, p.redact ?? false), p.format, p.now ?? new Date()));
  if (path !== null) p.announce(`Saved to ${path}.`);
}

/** Copies an export and says in which format. */
export async function copyExport(p: {
  text: string;
  format: ExportFormat;
  onCopy: (text: string) => Promise<void>;
  announce: (text: string) => void;
}): Promise<void> {
  await p.onCopy(p.text);
  p.announce(`Copied as ${formatName(p.format)}.`);
}

type Preview = { kind: "loading" } | { kind: "ready"; text: string } | { kind: "none" } | { kind: "failed"; message: string };

const FORMATS = [
  ["markdown", "Markdown"],
  ["json", "JSON"],
  ["asciicast", "Asciicast (.cast)"],
] as const;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** What a failed load comes to: for an asciicast, a 404 means the conversation has no recording, which is an answer, not a failure. */
export function previewFailure(format: ExportFormat, e: unknown): Preview {
  return format === "asciicast" && isNotFound(e) ? { kind: "none" } : { kind: "failed", message: message(e) };
}

export type ExportDialogProps = {
  /** The conversation's title, which names the saved file. */
  title: string;
  /** The conversation as text in a format, with secrets and personal paths hidden when `redact`. */
  load: (format: ExportFormat, redact: boolean) => Promise<string>;
  /** Saves through the native Save dialog: the path it was saved to, or null when the person cancelled. */
  onSave: (text: string, name: string) => Promise<string | null>;
  onCopy: (text: string) => Promise<void>;
  onClose: () => void;
  announce: (text: string) => void;
  /** The format selected when the dialog opens. */
  initialFormat?: ExportFormat;
};

/**
 * Export a conversation as markdown, JSON or an asciicast of its recording, with secrets and personal paths hidden unless the box is cleared. A modal dialog
 * like Diagnostics: focus starts on the title, Tab stays inside, Esc closes and focus goes back to whatever opened it. The
 * preview is the text that Save and Copy will use, and loads again when the format or the box changes. A button that is busy
 * is `aria-disabled`, not `disabled`, so it keeps focus.
 */
export function ExportDialog({ title, load, onSave, onCopy, onClose, announce, initialFormat = "markdown" }: ExportDialogProps) {
  const uid = useId();
  const dialog = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [format, setFormat] = useState<ExportFormat>(initialFormat);
  const [redact, setRedact] = useState(true);
  const [preview, setPreview] = useState<Preview>({ kind: "loading" });
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const loadRef = useRef(load);
  loadRef.current = load;
  useDialogFocus(dialog, heading, onClose);

  useEffect(() => {
    let live = true;
    setPreview({ kind: "loading" });
    setProblem(null);
    loadRef.current(format, redact).then(
      (text) => live && setPreview({ kind: "ready", text }),
      (e: unknown) => live && setPreview(previewFailure(format, e)),
    );
    return () => {
      live = false;
    };
  }, [format, redact]);

  // The window's announcer says a failure; the line in the dialog is for the eyes.
  const loadFailure = preview.kind === "failed" ? `Couldn't prepare the export: ${preview.message}` : null;
  useEffect(() => {
    if (loadFailure) announce(loadFailure);
  }, [loadFailure, announce]);

  const text = preview.kind === "ready" ? preview.text : null;
  const act = async (what: "save" | "copy") => {
    if (text === null || busy) return;
    setBusy(true);
    setProblem(null);
    try {
      if (what === "save") await saveExport({ text, title, format, redact, onSave, announce });
      else await copyExport({ text, format, onCopy, announce });
    } catch (e) {
      const failed = `Couldn't ${what === "save" ? "save" : "copy"} the export: ${message(e)}`;
      setProblem(failed);
      announce(failed);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="export-dialog">
      <div className="export-dialog__backdrop" onClick={onClose} />
      <div ref={dialog} className="export-dialog__sheet" role="dialog" aria-modal="true" aria-label="Export conversation">
        <h2 ref={heading} tabIndex={-1} className="export-dialog__title">
          Export conversation
        </h2>
        <div className="export-dialog__body">
          <fieldset className="export-dialog__formats">
            <legend className="export-dialog__legend">Format</legend>
            {FORMATS.map(([f, label]) => (
              <label key={f} className="export-dialog__choice">
                <input type="radio" name={`${uid}-format`} value={f} checked={format === f} onChange={() => setFormat(f)} />
                <span>{label}</span>
              </label>
            ))}
          </fieldset>
          <label className="export-dialog__choice">
            <input type="checkbox" checked={redact} onChange={(e) => setRedact(e.target.checked)} />
            <span>Hide secrets and personal paths</span>
          </label>
          <p id={`${uid}-preview`} className="export-dialog__legend">
            Preview
          </p>
          <pre tabIndex={0} className="export-dialog__text" aria-labelledby={`${uid}-preview`} dir="auto">
            {preview.kind === "ready"
              ? format === "asciicast"
                ? firstLines(preview.text)
                : preview.text
              : preview.kind === "loading"
                ? "Loading…"
                : preview.kind === "none"
                  ? NO_RECORDING_COPY
                  : ""}
          </pre>
          {format === "asciicast" && preview.kind !== "none" && preview.kind !== "failed" && (
            <p className="export-dialog__legend">{`The preview shows the first ${PREVIEW_LINES} lines.`}</p>
          )}
          {loadFailure && <p className="export-dialog__error">{loadFailure}</p>}
          {problem && <p className="export-dialog__error">{problem}</p>}
        </div>
        <footer className="export-dialog__foot">
          <button type="button" className="export-dialog__btn" aria-disabled={text === null || busy || undefined} onClick={() => void act("save")}>
            Save…
          </button>
          {format !== "asciicast" && (
            <button type="button" className="export-dialog__btn" aria-disabled={text === null || busy || undefined} onClick={() => void act("copy")}>
              Copy
            </button>
          )}
          <button type="button" className="export-dialog__btn" onClick={onClose}>
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}
