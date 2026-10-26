import { useEffect, useRef, useState } from "react";
import { Switch } from "../../components/Switch/Switch";
import type { Endpoint } from "../../generated/Endpoint";
import type { ExportPreview } from "../../generated/ExportPreview";
import type { LoggingState } from "../../generated/LoggingState";
import { ApiError } from "../../lib/api/client";
import { diagnostics, exportName, formatBytes, saveExport } from "../../lib/api/diagnostics";
import { useAnnounceOnce } from "../../lib/useAnnounceOnce";
import "./AdvancedSection.css";

export const EXPIRED = "This export has expired. Prepare it again.";
export const LOGGING_UNREADABLE = "Couldn't read the log setting from Marshell's core. Close this and try again.";
export const PREPARE_FAILED = "Couldn't prepare the export. Try again.";

export function exportReadyMessage(preview: ExportPreview): string {
  return `Export ready: ${preview.files.length} ${preview.files.length === 1 ? "file" : "files"}, ${formatBytes(preview.total_bytes)}.`;
}

type PreviewState = ExportPreview | "idle" | "preparing" | "expired" | "failed";

/** User-requested export transitions can replace the focused control. Unrelated renders leave focus alone. */
export function diagnosticsFocusTarget(previous: PreviewState, next: PreviewState): "diagnostics-prepare" | "diagnostics-export-heading" | null {
  if (previous === next) return null;
  if (typeof next === "object") return "diagnostics-export-heading";
  if (next === "preparing" || next === "expired" || next === "failed") return "diagnostics-prepare";
  return null;
}

export type DiagnosticsHandlers = {
  onDetailed: (checked: boolean) => void;
  onPrepare: () => void;
  onView: (name: string) => void;
  /** Leaves the viewer for the list of files. */
  onBack: () => void;
  onSave: () => void;
};

/**
 * Settings → Advanced: the detailed-logs switch and a previewed export. Presentational:
 * `AdvancedSection` fetches and acts, this draws. Focus: after an export is prepared the export heading; opening a file moves it to the text, and
 * Back to the files moves it to that file's View button. A button that is busy (Prepare while it builds, Save while the native
 * dialog is open) is `aria-disabled`, not `disabled`: a disabled button that has focus drops it to the page, and then Escape
 * and Tab no longer reach the page. The handlers ignore a press while busy.
 */
export function AdvancedView({
  logging,
  preview,
  viewing,
  saving = false,
  ...on
}: {
  logging: LoggingState | "loading" | "failed";
  preview: PreviewState;
  viewing: { name: string; text: string } | null;
  saving?: boolean;
} & DiagnosticsHandlers) {
  const root = useRef<HTMLDivElement>(null);
  const text = useRef<HTMLPreElement>(null);
  const viewButtons = useRef(new Map<string, HTMLButtonElement>());
  const lastViewed = useRef<string | null>(null);
  const previousPreview = useRef(preview);

  const viewingName = viewing?.name ?? null;
  useEffect(() => {
    const target = diagnosticsFocusTarget(previousPreview.current, preview);
    previousPreview.current = preview;
    if (target !== null) {
      lastViewed.current = null;
      root.current?.querySelector<HTMLElement>(`#${target}`)?.focus();
      return;
    }
    if (viewingName !== null) {
      lastViewed.current = viewingName;
      text.current?.focus();
    } else if (lastViewed.current !== null) {
      viewButtons.current.get(lastViewed.current)?.focus();
      lastViewed.current = null;
    }
  }, [preview, viewingName]);

  const files = typeof preview === "object" ? preview : null;
  return (
    <div ref={root} className="advanced">
          <section className="diagnostics-dialog__section" aria-label="Logs">
            {logging === "loading" && <p className="diagnostics-dialog__line">Reading the log setting…</p>}
            {logging === "failed" && (
              <p className="diagnostics-dialog__error">{LOGGING_UNREADABLE}</p>
            )}
            {typeof logging === "object" && (
              <Switch
                label="Detailed logs"
                checked={logging.available && logging.detailed}
                disabled={!logging.available}
                onChange={on.onDetailed}
                reason={logging.available ? "Writes debug lines to Marshell's log until Marshell restarts." : "Logging isn't running in this build."}
              />
            )}
          </section>
          <section className="diagnostics-dialog__section" aria-labelledby="diagnostics-export-heading">
            <h3 id="diagnostics-export-heading" tabIndex={-1} className="diagnostics-dialog__name">
              Export diagnostics
            </h3>
            <p className="diagnostics-dialog__line">Logs and setup only: no transcripts, recordings, prompts or hook events. Keys, tokens, your home folder and user name are replaced.</p>
            {(preview === "preparing" || preview === "expired") && (
              <p className={preview === "expired" ? "diagnostics-dialog__error" : "diagnostics-dialog__line"}>{preview === "preparing" ? "Preparing the export…" : EXPIRED}</p>
            )}
            {viewing ? (
              <div className="diagnostics-dialog__viewer">
                <h4 id="diagnostics-file-heading" className="diagnostics-dialog__file-name" dir="auto">
                  {viewing.name}
                </h4>
                <pre ref={text} tabIndex={0} className="diagnostics-dialog__text" aria-labelledby="diagnostics-file-heading">
                  {viewing.text}
                </pre>
                <div className="diagnostics-dialog__actions">
                  <button type="button" className="diagnostics-dialog__btn" onClick={on.onBack}>
                    Back to the files
                  </button>
                </div>
              </div>
            ) : (
              <>
                {preview === "failed" && (
                  <p className="diagnostics-dialog__error">{PREPARE_FAILED}</p>
                )}
                {files && (
                  <>
                    <ul className="diagnostics-dialog__files" aria-label="Files in the export">
                      {files.files.map((f) => (
                        <li key={f.name} className="diagnostics-dialog__file">
                          <span className="diagnostics-dialog__file-label" dir="auto">
                            {f.name}
                          </span>
                          <span className="diagnostics-dialog__size">{formatBytes(f.bytes)}</span>
                          <button
                            type="button"
                            className="diagnostics-dialog__btn"
                            aria-label={`View ${f.name}`}
                            ref={(el) => {
                              if (el) viewButtons.current.set(f.name, el);
                              else viewButtons.current.delete(f.name);
                            }}
                            onClick={() => on.onView(f.name)}
                          >
                            View
                          </button>
                        </li>
                      ))}
                    </ul>
                    <p className="diagnostics-dialog__line">
                      {files.files.length} {files.files.length === 1 ? "file" : "files"}, {formatBytes(files.total_bytes)}
                    </p>
                  </>
                )}
                <div className="diagnostics-dialog__actions">
                  {files && (
                    <button type="button" className="diagnostics-dialog__btn" aria-disabled={saving || undefined} onClick={on.onSave}>
                      Save…
                    </button>
                  )}
                  <button id="diagnostics-prepare" type="button" className="diagnostics-dialog__btn" aria-disabled={preview === "preparing" || undefined} onClick={on.onPrepare}>
                    {files ? "Prepare again" : "Prepare export"}
                  </button>
                </div>
              </>
            )}
          </section>
    </div>
  );
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * The Advanced section on the core's data. Nothing polls: the logging state is read when it opens, and the export is built
 * only when asked for. The zip is fetched from the core and written by the app's native Save dialog.
 */
export function AdvancedSection({ endpoint, announce }: { endpoint: Endpoint; announce?: (text: string) => void }) {
  const [logging, setLogging] = useState<LoggingState | "loading" | "failed">("loading");
  const [preview, setPreview] = useState<PreviewState>("idle");
  const [viewing, setViewing] = useState<{ name: string; text: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const exportId = typeof preview === "object" ? preview.id : null;
  // The view draws these two as plain text; the page's announcer says each once, when it appears.
  useAnnounceOnce(logging === "failed" ? LOGGING_UNREADABLE : null, announce);
  useAnnounceOnce(preview === "failed" ? PREPARE_FAILED : null, announce);

  useEffect(() => {
    let live = true;
    diagnostics.logging(endpoint).then(
      (s) => live && setLogging(s),
      () => live && setLogging("failed"),
    );
    return () => {
      live = false;
    };
  }, [endpoint]);

  const detailed = async (on: boolean) => {
    try {
      setLogging(await diagnostics.setDetailed(endpoint, on));
    } catch (e) {
      announce?.(`Couldn't change the log level: ${message(e)}`);
      diagnostics.logging(endpoint).then(setLogging, () => {});
    }
  };

  const prepare = async () => {
    if (preview === "preparing") return;
    setViewing(null);
    setPreview("preparing");
    announce?.("Preparing the export…");
    try {
      const next = await diagnostics.prepare(endpoint);
      setPreview(next);
      announce?.(exportReadyMessage(next));
    } catch {
      setPreview("failed");
    }
  };

  const expired = (e: unknown) => {
    if (!(e instanceof ApiError && e.status === 404)) return false;
    setViewing(null);
    setPreview("expired");
    announce?.(EXPIRED);
    return true;
  };

  const view = async (name: string) => {
    if (!exportId) return;
    try {
      setViewing({ name, text: await diagnostics.file(endpoint, exportId, name) });
    } catch (e) {
      if (!expired(e)) announce?.(`Couldn't open ${name}: ${message(e)}`);
    }
  };

  const save = async () => {
    if (!exportId || saving) return;
    setSaving(true);
    try {
      const path = await saveExport(await diagnostics.zip(endpoint, exportId), exportName(new Date()));
      if (path !== null) announce?.(`Saved to ${path}`);
    } catch (e) {
      if (!expired(e)) announce?.(`Couldn't save the export: ${message(e)}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <AdvancedView
      logging={logging}
      preview={preview}
      viewing={viewing}
      saving={saving}
      onDetailed={(on) => void detailed(on)}
      onPrepare={() => void prepare()}
      onView={(name) => void view(name)}
      onBack={() => setViewing(null)}
      onSave={() => void save()}
    />
  );
}
