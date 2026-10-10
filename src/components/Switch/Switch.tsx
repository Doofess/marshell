import { useId } from "react";
import "./Switch.css";

/** A native switch: the state is also a word (On / Off), so it never depends on colour. When unavailable, the reason is its description. */
export function Switch({ label, checked, disabled = false, reason }: { label: string; checked: boolean; disabled?: boolean; reason?: string }) {
  const id = useId();
  const why = `${id}-why`;
  return (
    <div className="switch" data-disabled={disabled || undefined}>
      <label className="switch__label">
        <input type="checkbox" role="switch" className="switch__input" defaultChecked={checked} disabled={disabled} aria-describedby={reason ? why : undefined} />
        <span className="switch__track" aria-hidden="true" />
        <span className="switch__text">{label}</span>
        <span className="switch__state">{checked ? "On" : "Off"}</span>
      </label>
      {reason && (
        <p id={why} className="switch__reason">
          {reason}
        </p>
      )}
    </div>
  );
}
