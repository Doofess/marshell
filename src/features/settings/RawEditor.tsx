import { useId } from "react";
import { parseRaw } from "./raw";
import "./Settings.css";

/**
 * The file as text. A mistake is named by line and column and linked to the text area (aria-describedby), so a screen
 * reader says it on arrival. In Phase 0 this is strict JSON; the core's jsonc reader replaces it in phase 3. A file that must
 * not be edited (read-only, or does not parse) is shown read-only.
 */
export function RawEditor({ path, text, readOnly = false }: { path: string; text: string; readOnly?: boolean }) {
  const id = useId();
  const r = parseRaw(text);
  const rows = Math.min(24, Math.max(6, text.split("\n").length));
  return (
    <div className="raw-editor">
      <textarea
        className="raw-editor__text"
        aria-label={path}
        rows={rows}
        spellCheck={false}
        defaultValue={text}
        readOnly={readOnly}
        aria-invalid={r.ok ? undefined : true}
        aria-describedby={r.ok ? undefined : id}
      />
      {!r.ok && (
        <p id={id} className="raw-editor__error">
          {`Line ${r.line}, column ${r.column}: ${r.message}`}
        </p>
      )}
    </div>
  );
}
