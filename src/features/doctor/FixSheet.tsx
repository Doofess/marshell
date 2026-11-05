import { useId, useRef } from "react";
import { DiffView } from "../../components/DiffView/DiffView";
import { deletedFiles } from "../../lib/api/agents";
import { useDialogFocus } from "../../lib/useDialogFocus";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import { fixesCodexHooks, fixHeadline, restartNote } from "./report";
import type { DoctorFixPlan } from "./types";
import "./Doctor.css";

function FileDiffs({ preview, prefix }: { preview: ConfigPreview; prefix: string }) {
  const deleted = deletedFiles(preview);
  return (
    <>
      <div className="fix-sheet__groups">
        {preview.files
          .filter((f) => f.diff !== "")
          .map((f) => (
            <div key={f.path} className="fix-sheet__group">
              <p className="fix-sheet__file">
                <code dir="auto">{f.path}</code>
              </p>
              <DiffView diff={f.diff} label={`${prefix} ${f.path}`} />
            </div>
          ))}
      </div>
      {deleted.map((p) => (
        <p key={p} className="fix-sheet__note">
          Deletes <code dir="auto">{p}</code>.
        </p>
      ))}
    </>
  );
}

/**
 * The sheet before a fix: the exact change to each file, the backup, how to undo and how many sessions need a restart.
 * Focus lands on the title, not on an action, so Enter cannot apply anything by reflex. In the app (useDialogFocus) Tab stays
 * inside, Escape calls onCancel and focus returns to whatever opened the sheet. The changes scroll in the body, which is itself
 * a focusable region (so the keyboard can scroll it, and the trap reaches it); the actions sit below it, so they are
 * reachable however long the change is. When a file changed after the preview, nothing is applied: Reload or Cancel only.
 */
export function FixSheet({ plan, busy, conflict, onConfirm, onCancel, onReload }: { plan: DoctorFixPlan; busy: boolean; conflict: boolean; onConfirm: () => void; onCancel: () => void; onReload: () => void }) {
  const id = useId();
  const dialog = useRef<HTMLElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  useDialogFocus(dialog, title, onCancel);
  const headline = fixHeadline(plan.item_ids.length);
  return (
    <section ref={dialog} className="fix-sheet" role="dialog" aria-modal="true" aria-labelledby={id}>
      <h2 id={id} ref={title} className="fix-sheet__title" tabIndex={-1}>
        {headline}
      </h2>
      <div className="fix-sheet__body" role="region" aria-label="Changes to review" tabIndex={0}>
        {conflict && (
          <p className="fix-sheet__conflict">A file changed after this preview was made, so nothing was written. Reload to see what it says now.</p>
        )}
        <FileDiffs preview={plan.preview} prefix="Exact changes to" />
        <p className="fix-sheet__note">
          A backup is saved first, in <code>~/.marshell/snapshots/</code>. To undo: Undo last fix in the Doctor header puts that backup back, for as long as the files are unchanged since this fix.
        </p>
        {plan.restart.length > 0 && <p className="fix-sheet__note">{restartNote(plan.restart.length)}.</p>}
        {fixesCodexHooks(plan.item_ids) && (
          <p className="fix-sheet__note">
            Codex runs changed hooks only after you trust them. The next time Codex starts, choose <strong>Trust all and continue</strong>, or review them first in <code>/hooks</code>.
          </p>
        )}
      </div>
      <footer className="fix-sheet__foot">
        <button type="button" className="fix-sheet__btn" onClick={onCancel}>
          Cancel
        </button>
        {conflict ? (
          <button type="button" className="fix-sheet__btn" data-primary onClick={onReload}>
            Reload
          </button>
        ) : (
          <button type="button" className="fix-sheet__btn" data-primary aria-disabled={busy ? "true" : undefined} onClick={busy ? undefined : onConfirm}>
            {busy ? "Fixing…" : headline}
          </button>
        )}
      </footer>
    </section>
  );
}

/** "Show differences": what the files say now against the fix, for when undo is withdrawn. Read-only, closed with Close or Escape. */
export function DifferencesSheet({ preview, onClose }: { preview: ConfigPreview; onClose: () => void }) {
  const id = useId();
  const dialog = useRef<HTMLElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  useDialogFocus(dialog, title, onClose);
  return (
    <section ref={dialog} className="fix-sheet" role="dialog" aria-modal="true" aria-labelledby={id}>
      <h2 id={id} ref={title} className="fix-sheet__title" tabIndex={-1}>
        {preview.title || "Differences since the fix"}
      </h2>
      <div className="fix-sheet__body" role="region" aria-label="Differences" tabIndex={0}>
        {preview.reason && <p className="fix-sheet__note">{preview.reason}</p>}
        <FileDiffs preview={preview} prefix="Differences in" />
      </div>
      <footer className="fix-sheet__foot">
        <button type="button" className="fix-sheet__btn" data-primary onClick={onClose}>
          Close
        </button>
      </footer>
    </section>
  );
}
