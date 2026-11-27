import { useId } from "react";
import type { CopyRow } from "../../generated/CopyRow";
import { copyFixes, rowLabel, stateText } from "./instructionsModel";

/**
 * One row per place a doc is copied to: the CLI (and distro), the state in words, the file, and each note on its own line.
 * A copy the doc wants that was changed outside Marshell offers the two fixes; an orphan row never does.
 */
export function LoadsIn({ rows, orphan = false, onCopyFix }: { rows: CopyRow[]; orphan?: boolean; onCopyFix?: (row: CopyRow, takeCopy: boolean) => void }) {
  const uid = useId();
  return (
    <ul className="loads">
      {rows.map((row, i) => {
        const hint = `${uid}-${i}`;
        return (
          <li key={`${row.cli ?? ""}/${row.distro ?? ""}/${row.path ?? ""}`} className="loads__row" data-state={row.state}>
            <p className="loads__head">
              <span className="loads__cli">{rowLabel(row)}</span>
              <span className="loads__state">{stateText(row)}</span>
            </p>
            {row.path !== undefined && (
              <p className="loads__path">
                <code dir="auto">{row.path}</code>
              </p>
            )}
            {row.notes.map((n) => (
              <p key={n} className="loads__note">
                {n}
              </p>
            ))}
            {copyFixes(row, orphan) && (
              <div className="loads__fixes">
                <button type="button" className="instructions__btn" onClick={() => onCopyFix?.(row, false)}>
                  Put the doc's text back
                </button>
                <button type="button" className="instructions__btn" aria-describedby={hint} onClick={() => onCopyFix?.(row, true)}>
                  Use this copy's text
                </button>
                <span id={hint} className="instructions__hint">
                  Takes the text, not the frontmatter.
                </span>
              </div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
