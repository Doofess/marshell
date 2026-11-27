import { useId, useRef } from "react";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { useCoverPage } from "../browser/pageCover";
import { FileDiffs } from "../doctor/FixSheet";

/**
 * The sheet before the copies change (Review, a delete, or a fix of one copy): the exact change to each file. Focus lands on
 * the title, never on Apply. A changed file stops Apply: Reload or Cancel only. A preview with no file has nothing to apply,
 * and one the core can't apply (a refused file, said by path) greys Apply out.
 */
export function CopiesReview({ title, preview, busy, conflict, onApply, onCancel, onReload }: { title: string; preview: ConfigPreview; busy: boolean; conflict: boolean; onApply: () => void; onCancel: () => void; onReload: () => void }) {
  const id = useId();
  const dialog = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  useCoverPage();
  useDialogFocus(dialog, heading, onCancel);
  const nothing = preview.files.length === 0;
  return (
    <section ref={dialog} className="fix-sheet" role="dialog" aria-modal="true" aria-labelledby={id}>
      <h2 id={id} ref={heading} className="fix-sheet__title" tabIndex={-1}>
        {title}
      </h2>
      <div className="fix-sheet__body" role="region" aria-label="Changes to review" tabIndex={0}>
        {conflict && <p className="fix-sheet__conflict">A file changed since this preview. Reload it to see the change.</p>}
        {nothing ? <p className="fix-sheet__note">Nothing to update: the copies are up to date.</p> : <FileDiffs preview={preview} prefix="Exact changes to" />}
        {preview.files.map((f) =>
          f.status.kind === "refused" ? (
            <p key={`refused-${f.path}`} className="fix-sheet__note">
              Marshell will not edit <code dir="auto">{f.path}</code>: {f.status.detail}
            </p>
          ) : null,
        )}
      </div>
      <footer className="fix-sheet__foot">
        {nothing ? (
          <button type="button" className="fix-sheet__btn" data-primary onClick={onCancel}>
            Close
          </button>
        ) : (
          <>
            <button type="button" className="fix-sheet__btn" onClick={onCancel}>
              Cancel
            </button>
            {conflict ? (
              <button type="button" className="fix-sheet__btn" data-primary onClick={onReload}>
                Reload
              </button>
            ) : (
              <button type="button" className="fix-sheet__btn" data-primary aria-disabled={busy || !preview.can_apply ? "true" : undefined} onClick={busy || !preview.can_apply ? undefined : onApply}>
                {busy ? "Applying…" : "Apply"}
              </button>
            )}
          </>
        )}
      </footer>
    </section>
  );
}
