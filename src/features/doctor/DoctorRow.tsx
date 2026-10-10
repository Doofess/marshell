import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { listNumbers } from "./report";
import type { DoctorItem, Severity } from "./types";
import "./Doctor.css";

const WORD: Record<Severity, string> = { error: "Error", warning: "Warning", info: "Information" };

function Mark({ severity }: { severity: Severity }) {
  if (severity === "error") return <StatusGlyph kind="error" size={16} label="" />;
  if (severity === "warning") return <AuxGlyph kind="caution" label="" />;
  return (
    <span className="doctor__info-mark" aria-hidden="true">
      i
    </span>
  );
}

/** The restart sentence for the sessions that only pick a fix up after a restart, naming them by number. */
export function restartNote(n: number[]): string {
  return `${n.length === 1 ? "Session" : "Sessions"} ${listNumbers(n)} ${n.length === 1 ? "picks" : "pick"} this up after a restart`;
}

/** One finding: a glyph and the severity in words, the title, what it means, the file, and why it cannot be fixed when it cannot. */
export function DoctorRow({ item, fixed = false }: { item: DoctorItem; fixed?: boolean }) {
  const done = fixed && item.fixable;
  return (
    <li className="doctor__row" data-severity={item.severity} data-state={done ? "fixed" : undefined}>
      <span className="doctor__mark">
        <Mark severity={item.severity} />
      </span>
      <div className="doctor__main">
        <p className="doctor__title">
          <span className="doctor__word">{done ? "Fixed" : WORD[item.severity]}</span> <span dir="auto">{item.title}</span>
        </p>
        <p className="doctor__detail" dir="auto">
          {item.detail}
        </p>
        {item.file && (
          <p className="doctor__file">
            <code>{item.file}</code>
          </p>
        )}
        {item.restartSessions && item.restartSessions.length > 0 && <p className="doctor__restart">{restartNote(item.restartSessions)}.</p>}
        {!item.fixable && item.severity !== "info" && item.unfixableReason && <p className="doctor__why">{item.unfixableReason}</p>}
      </div>
    </li>
  );
}
