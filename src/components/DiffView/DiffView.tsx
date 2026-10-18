import { useState } from "react";
import { collapse, parseUnifiedDiff, SIGN } from "./diff";
import "./DiffView.css";

type Props = { diff: string; label: string; limit?: number; expanded?: boolean };

/** A unified diff, read-only: every changed line has a sign as well as a fill, and a long one folds after `limit` lines. */
export function DiffView({ expanded = false, ...rest }: Props) {
  const [open, setOpen] = useState(false);
  return <DiffBody {...rest} open={expanded || open} onToggle={expanded ? undefined : () => setOpen((o) => !o)} />;
}

/** The diff as shown. With `onToggle`, a long diff has a button that opens and folds it, and stays put so focus does not drop. */
export function DiffBody({ diff, label, limit = 40, open, onToggle }: Omit<Props, "expanded"> & { open: boolean; onToggle?: () => void }) {
  const p = parseUnifiedDiff(diff);
  const { shown } = open ? { shown: p.lines } : collapse(p.lines, limit);
  const foldable = onToggle !== undefined && p.lines.length > limit;
  return (
    <figure className="diff" aria-label={label}>
      <figcaption className="diff__head">
        <span className="diff__file" dir="auto">
          {p.files.join(", ") || (p.lines.length === 0 ? "No changes" : "Changes")}
        </span>
        {p.lines.length > 0 && (
          <span className="diff__count">
            <span className="diff__plus">+{p.added}</span> <span className="diff__minus">−{p.removed}</span>
          </span>
        )}
      </figcaption>
      <ol className="diff__lines">
        {shown.map((l, i) => (
          <li key={i} className="diff__line" data-kind={l.kind}>
            <span className="diff__no" aria-hidden="true">
              {l.newNo ?? l.oldNo ?? ""}
            </span>
            <span className="diff__sign" aria-hidden="true">
              {SIGN[l.kind]}
            </span>
            {l.kind === "add" && <span className="diff__sr">added </span>}
            {l.kind === "del" && <span className="diff__sr">removed </span>}
            <code dir="ltr">{l.text}</code>
          </li>
        ))}
      </ol>
      {foldable && (
        <button type="button" className="diff__more" aria-expanded={open} onClick={onToggle}>
          {open ? "Show fewer lines" : `Show all ${p.lines.filter((l) => l.kind !== "meta" && l.kind !== "hunk").length} lines`}
        </button>
      )}
    </figure>
  );
}
