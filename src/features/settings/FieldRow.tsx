import { useId } from "react";
import { Switch } from "../../components/Switch/Switch";
import { appliesNote, isEditable, type FileState, type SettingField } from "./fields";
import { ScopeBadge } from "./ScopeBadge";
import "./Settings.css";

/**
 * One setting: its name and where it comes from (scope badge), what it does, the control, and when a change takes effect.
 * A managed value, or any value in a file that must not be edited, has a disabled control and says why in words. The badge
 * shows where the value is really read from, so a value a managed file locks says Managed, and the control shows the value the
 * organisation enforces (the one that applies), not the user's own. When the file cannot be read, a value is an en dash, never
 * a guess from what the broken file used to say: every kind, a boolean included, shows the en dash text control then, since
 * an off switch would read as a value.
 */
export function FieldRow({ field, file = {}, value }: { field: SettingField; file?: FileState; value?: unknown }) {
  const id = useId();
  const can = isEditable(field, file);
  const locked = !can.ok;
  const managed = Boolean(field.lockedBy) && field.managedValue !== undefined;
  const unknown = Boolean(file.unparsable) && !managed;
  const shown = managed ? field.managedValue : value === undefined ? field.value : value;
  const why = managed ? `Your organisation sets this to ${String(field.managedValue)}; your own value (${String(field.value)}) is not used.` : undefined;
  return (
    <li className="field" data-locked={locked || undefined}>
      <div className="field__head">
        {field.kind === "boolean" && !unknown ? (
          <Switch label={field.label} checked={Boolean(shown)} disabled={locked} />
        ) : (
          <label htmlFor={id} className="field__label" dir="auto">
            {field.label}
          </label>
        )}
        <ScopeBadge scope={field.lockedBy ?? field.scope} />
      </div>
      <p className="field__help" dir="auto">
        {field.help}
      </p>
      {unknown && <input id={id} className="field__control" type="text" value="–" readOnly disabled data-unknown="" />}
      {!unknown && field.kind === "enum" && (
        <select id={id} className="field__control" defaultValue={String(shown ?? "")} disabled={locked}>
          {(field.options ?? []).map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
      )}
      {!unknown && field.kind === "number" && <input id={id} className="field__control" type="number" defaultValue={shown === undefined ? undefined : Number(shown)} placeholder="–" disabled={locked} />}
      {!unknown && field.kind === "text" && <input id={id} className="field__control" type="text" defaultValue={String(shown ?? "")} placeholder="–" disabled={locked} />}
      {locked && (
        <p className="field__why">
          {can.reason}
          {why && ` ${why}`}
        </p>
      )}
      <p className="field__applies">{appliesNote(field)}</p>
    </li>
  );
}
