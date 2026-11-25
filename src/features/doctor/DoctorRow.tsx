import { type CSSProperties, Fragment, useId } from "react";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { keepFocusKey, keepLabel, keepName } from "./keep";
import type { DoctorGroup, DoctorItem, DoctorOpen } from "./types";
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

type Props = { item: DoctorItem; fixed?: boolean; checked?: boolean; disabled?: boolean; fixing?: boolean; keeping?: boolean; onToggle?: (id: string) => void; onOpenAgents?: () => void; onOpenFile?: (open: DoctorOpen) => void; onKeep?: (item: DoctorItem) => void };

/**
 * One finding: a glyph, the title (with a checkbox when Marshell can fix it), what it means, the file, and why it cannot be
 * fixed when it cannot. The group heading above the row says Broken, Waste or Info; a fixed row says Fixed in words.
 * An instruction line also opens to the files each CLI loads. The summary is described by the row's title, so a screen
 * reader tells thirty "Show the files" apart. A finding about files has a button for each, which opens it in the side pane.
 */
export function DoctorRow({ item, fixed = false, checked = false, disabled = false, fixing = false, keeping = false, onToggle, onOpenAgents, onOpenFile, onKeep }: Props) {
  const titleId = useId();
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
        <p className="doctor__title" id={titleId}>
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
        {item.loads && item.loads.length > 0 && (
          <details className="doctor__loads">
            <summary aria-describedby={titleId}>Show the files</summary>
            {item.loads.map((load) => (
              <Fragment key={load.agent}>
                <p className="doctor__load-cli">{load.heading}</p>
                {load.notes?.map((note, i) => (
                  <p className="doctor__load-note" key={i}>
                    {note}
                  </p>
                ))}
                <ul className="doctor__load-files">
                  {load.files.map((file, i) => (
                    // The note's words say when the file loads; the colour only repeats it.
                    <li key={i} data-when={file.when} style={{ "--depth": file.depth } as CSSProperties}>
                      <code dir="ltr">{file.path}</code> <span>{file.note}</span>
                    </li>
                  ))}
                </ul>
              </Fragment>
            ))}
          </details>
        )}
        {item.opens && item.opens.length > 0 && (
          <div className="doctor__opens">
            {item.opens.map((open, i) => (
              <button type="button" className="doctor__btn" key={i} aria-describedby={titleId} onClick={() => onOpenFile?.(open)}>
                {open.label}
              </button>
            ))}
          </div>
        )}
        {item.open_agents && (
          <div>
            <button type="button" className="doctor__btn" onClick={onOpenAgents}>
              Open Agents
            </button>
          </div>
        )}
        {item.action && (
          <div>
            {/* aria-disabled, not disabled: the pressed button keeps the focus while its request runs. Off too while a fix is applied. */}
            <button type="button" className="doctor__btn" data-doctor-keep={keepFocusKey(item)} aria-label={keepName(item.action, item.title, keeping)} aria-disabled={keeping || fixing ? "true" : undefined} onClick={keeping || fixing ? undefined : () => onKeep?.(item)}>
              {keepLabel(item.action, keeping)}
            </button>
          </div>
        )}
      </div>
    </li>
  );
}
