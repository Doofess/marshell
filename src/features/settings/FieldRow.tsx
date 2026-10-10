import { useId } from "react";
import { Switch } from "../../components/Switch/Switch";
import { appliesNote, isEditable, type FileState, type SettingField } from "./fields";
import { ScopeBadge } from "./ScopeBadge";
import "./Settings.css";

/**
 * One setting: its name and where it comes from (scope badge), what it does, the control, and when a change takes effect.
 * A managed value, or any value in a file that must not be edited, has a disabled control and says why in words.
 */
export function FieldRow({ field, file = {}, value }: { field: SettingField; file?: FileState; value?: unknown }) {
  const id = useId();
  const can = isEditable(field, file);
  const locked = !can.ok;
  const shown = value === undefined ? field.value : value;
  return (
    <li className="field" data-locked={locked || undefined}>
      <div className="field__head">
        {field.kind === "boolean" ? (
          <Switch label={field.label} checked={Boolean(shown)} disabled={locked} />
        ) : (
          <label htmlFor={id} className="field__label" dir="auto">
            {field.label}
          </label>
        )}
        <ScopeBadge scope={field.scope} />
      </div>
      <p className="field__help" dir="auto">
        {field.help}
      </p>
      {field.kind === "enum" && (
        <select id={id} className="field__control" defaultValue={String(shown ?? "")} disabled={locked}>
          {(field.options ?? []).map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
      )}
      {field.kind === "number" && <input id={id} className="field__control" type="number" defaultValue={Number(shown ?? 0)} disabled={locked} />}
      {field.kind === "text" && <input id={id} className="field__control" type="text" defaultValue={String(shown ?? "")} disabled={locked} />}
      {locked && <p className="field__why">{can.reason}</p>}
      <p className="field__applies">{appliesNote(field)}</p>
    </li>
  );
}
