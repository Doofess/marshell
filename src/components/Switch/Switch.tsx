import { useId } from "react";
import "./Switch.css";

/**
 * A native switch. Its state is also written as a word (On / Off), so it never depends on colour; the word is decoration for
 * the eyes (aria-hidden: a switch already says on or off) and follows the control's own checked state through CSS, so it stays
 * right after a toggle. When unavailable, the reason is its description. A value that cannot be read is never a switch: the
 * field row shows an en dash control instead, so an off switch never stands for "unknown".
 */
export function Switch({ label, checked, disabled = false, reason }: { label: string; checked: boolean; disabled?: boolean; reason?: string }) {
  const id = useId();
  const why = `${id}-why`;
  return (
    <div className="switch" data-disabled={disabled || undefined}>
      <label className="switch__label">
        <input type="checkbox" role="switch" className="switch__input" defaultChecked={checked} disabled={disabled} aria-describedby={reason ? why : undefined} />
        <span className="switch__track" aria-hidden="true" />
        <span className="switch__text">{label}</span>
        <span className="switch__state" aria-hidden="true">
          <span className="switch__on">On</span>
          <span className="switch__off">Off</span>
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
