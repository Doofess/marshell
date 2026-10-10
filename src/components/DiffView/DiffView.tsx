import { collapse, parseUnifiedDiff, SIGN } from "./diff";
import "./DiffView.css";

/** A unified diff, read-only: every changed line has a sign as well as a fill, and a long one folds after `limit` lines. */
export function DiffView({ diff, label, limit = 40, expanded = false }: { diff: string; label: string; limit?: number; expanded?: boolean }) {
  const p = parseUnifiedDiff(diff);
  const { shown, hidden } = expanded ? { shown: p.lines, hidden: 0 } : collapse(p.lines, limit);
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
            <span className="diff__sign">{SIGN[l.kind]}</span>
            {l.kind === "add" && <span className="diff__sr">added </span>}
            {l.kind === "del" && <span className="diff__sr">removed </span>}
            <code dir="auto">{l.text}</code>
          </li>
        ))}
      </ol>
      {hidden > 0 && (
        <button type="button" className="diff__more">
          Show all {p.lines.length} lines
        </button>
      )}
    </figure>
  );
}
