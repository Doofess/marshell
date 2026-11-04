import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import type { DoctorGroup, DoctorItem } from "./types";
import "./Doctor.css";

function Mark({ group, done }: { group: DoctorGroup; done: boolean }) {
  if (done) return <StatusGlyph kind="done-seen" size={16} label="" />;
  if (group === "broken") return <StatusGlyph kind="error" size={16} label="" />;
  if (group === "waste") return <AuxGlyph kind="caution" label="" />;
  return (
    <span className="doctor__info-mark" aria-hidden="true">
      i
    </span>
  );
}

type Props = { item: DoctorItem; fixed?: boolean; checked?: boolean; disabled?: boolean; onToggle?: (id: string) => void; onOpenAgents?: () => void };

/**
 * One finding: a glyph, the title (with a checkbox when Marshell can fix it), what it means, the file, and why it cannot be
 * fixed when it cannot. The group heading above the row says Broken, Waste or Info; a fixed row says Fixed in words.
 */
export function DoctorRow({ item, fixed = false, checked = false, disabled = false, onToggle, onOpenAgents }: Props) {
  const title = (
    <>
      {fixed && <span className="doctor__word">Fixed</span>} <span dir="auto">{item.title}</span>
    </>
  );
  return (
    <li className="doctor__row" data-group={item.group} data-state={fixed ? "fixed" : undefined}>
      <span className="doctor__mark" aria-hidden="true">
        <Mark group={item.group} done={fixed} />
      </span>
      <div className="doctor__main">
        <p className="doctor__title">
          {item.fixable && !fixed ? (
            <label className="doctor__check">
              <input type="checkbox" checked={checked} disabled={disabled} onChange={() => onToggle?.(item.id)} />
              <span>{title}</span>
            </label>
          ) : (
            title
          )}
        </p>
        <p className="doctor__detail" dir="auto">
          {item.detail}
        </p>
        {item.path && (
          <p className="doctor__file">
            <code>{item.path}</code>
          </p>
        )}
        {!item.fixable && item.cannot_fix && <p className="doctor__why">{item.cannot_fix}</p>}
        {item.open_agents && (
          <div>
            <button type="button" className="doctor__btn" onClick={onOpenAgents}>
              Open Agents
            </button>
          </div>
        )}
      </div>
    </li>
  );
}
