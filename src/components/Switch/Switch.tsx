import { useId } from "react";
import "./Switch.css";

/**
 * A native switch. Its state is also written as a word (On / Off), so it never depends on colour; the word is decoration for
 * the eyes (aria-hidden: a switch already says on or off) and follows the control's own checked state through CSS, so it stays
 * right after a toggle. When the state cannot be read it says so with an en dash. When unavailable, the reason is its description.
 */
export function Switch({ label, checked, disabled = false, reason, unknown = false }: { label: string; checked: boolean; disabled?: boolean; reason?: string; unknown?: boolean }) {
  const id = useId();
  const why = `${id}-why`;
  return (
    <div className="switch" data-disabled={disabled || undefined} data-unknown={unknown || undefined}>
      <label className="switch__label">
        <input type="checkbox" role="switch" className="switch__input" defaultChecked={checked && !unknown} disabled={disabled || unknown} aria-describedby={reason ? why : undefined} />
        <span className="switch__track" aria-hidden="true" />
        <span className="switch__text">{label}</span>
        <span className="switch__state" aria-hidden="true">
          <span className="switch__on">On</span>
          <span className="switch__off">Off</span>
          <span className="switch__unknown">–</span>
        </span>
      </label>
      {reason && (
        <p id={why} className="switch__reason">
          {reason}
        </p>
      )}
    </div>
  );
}
