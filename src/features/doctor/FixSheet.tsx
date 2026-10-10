import { useId } from "react";
import { DiffView } from "../../components/DiffView/DiffView";
import { restartNote } from "./DoctorRow";
import { counts, fixHeadline, groupByFile, restartSessionsOf } from "./report";
import type { DoctorReport } from "./types";
import "./Doctor.css";

/**
 * The sheet before a fix: the combined change grouped by file, the backup, how to undo and which sessions need a restart.
 * Focus lands on the title, not on an action, so Enter cannot apply anything by reflex; the dialog moves focus to the
 * title when it opens (tabindex -1) and keeps it inside until Cancel or the fix.
 */
export function FixSheet({ report }: { report: DoctorReport }) {
  const id = useId();
  const groups = groupByFile(report.items);
  const restart = restartSessionsOf(groups.flatMap((g) => g.items));
  const headline = fixHeadline(report);
  const n = counts(report).fixable;
  return (
    <section className="fix-sheet" role="dialog" aria-modal="true" aria-labelledby={id}>
      <h2 id={id} className="fix-sheet__title" tabIndex={-1}>
        {headline}
      </h2>
      <div className="fix-sheet__groups">
        {groups.map((g) => (
          <div key={g.file} className="fix-sheet__group">
            <p className="fix-sheet__file">
              <code dir="auto">{g.file}</code>
            </p>
            <DiffView diff={g.diff} label={`Changes to ${g.file}`} />
          </div>
        ))}
      </div>
      <p className="fix-sheet__note">
        A backup is saved first, in <code>~/.marshell/snapshots/</code>. To undo: Undo last fix in the Doctor header, for as long as the file still matches.
      </p>
      {restart.length > 0 && <p className="fix-sheet__note">{restartNote(restart)}.</p>}
      <footer className="fix-sheet__foot">
        <button type="button" className="fix-sheet__btn">
          Cancel
        </button>
        {n > 0 && (
          <button type="button" className="fix-sheet__btn" data-primary>
            {headline}
          </button>
        )}
      </footer>
    </section>
  );
}
