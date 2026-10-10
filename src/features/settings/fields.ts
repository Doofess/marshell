import { isReadOnlyScope, SCOPE_NOTE, type Scope } from "./scope";

export type FieldKind = "boolean" | "enum" | "number" | "text";
export type SettingField = {
  key: string;
  label: string;
  help: string;
  kind: FieldKind;
  options?: string[];
  value?: unknown;
  scope: Scope;
  /** The settings category (nav item) the field is listed under. */
  category: string;
  lockedBy?: "managed";
  applies: "live" | "new-sessions" | "restart";
};
export type FileState = { readOnly?: boolean; unparsable?: boolean };

/** Whether a field may be changed, and if not, the reason in words (shown beside it). */
export function isEditable(f: SettingField, file: FileState = {}): { ok: boolean; reason?: string } {
  if (isReadOnlyScope(f.scope) || f.lockedBy) return { ok: false, reason: SCOPE_NOTE.managed };
  if (file.unparsable) return { ok: false, reason: "This file does not parse, so Marshell won't edit it." };
  if (file.readOnly) return { ok: false, reason: "This file is read-only, so Marshell can't save changes." };
  return { ok: true };
}
export const appliesNote = (f: SettingField) => ({ live: "Applies at once.", "new-sessions": "Applies to new sessions.", restart: "Applies after you restart the session." })[f.applies];

/** The reason a whole file cannot be edited, shown once at the top of the page; undefined when it can be. */
export const fileReason = (file: FileState): string | undefined => (file.unparsable ? "This file does not parse, so Marshell won't edit it." : file.readOnly ? "This file is read-only, so Marshell can't save changes." : undefined);
