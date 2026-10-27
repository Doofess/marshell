import type { FieldControl } from "../../generated/FieldControl";
import type { SettingEdit } from "../../generated/SettingEdit";
import type { SettingValue } from "../../generated/SettingValue";
import { changeSummary } from "../settings/edit";
import { CHOICE_LABELS } from "./companyFields";

/** What each curated key's control shows, keyed by the key. `""` is Not set; a Switch uses `"on"` or `"off"`. */
export type Draft = Record<string, string>;

type Choice = Extract<FieldControl, { kind: "choice" }>;

/** The draft string for a value the file holds. */
export function draftValue(v: SettingValue): string {
  if (v.value === null) return "";
  if (v.control?.kind === "switch") return v.value === "true" ? "on" : v.value === "false" ? "off" : v.value;
  return v.value;
}

const SWITCH_OPTIONS = [
  { value: "", label: "Not set" },
  { value: "on", label: "On" },
  { value: "off", label: "Off" },
];

/** Not set, On and Off; a current value outside them comes next, marked as from the file. */
export const switchOptions = (current: string): { value: string; label: string }[] => (current === "" || current === "on" || current === "off" ? SWITCH_OPTIONS : [...SWITCH_OPTIONS, { value: current, label: `${current} (from the file)` }]);

/** The input as JSON text for the core (`null` for Not set), or the message for the field's error line. */
export function parseField(control: FieldControl, input: string): { ok: true; json: string | null } | { ok: false; message: string } {
  if (input === "") return { ok: true, json: null };
  switch (control.kind) {
    case "number": {
      const n = Number(input);
      if (!/^\d+$/.test(input.trim()) || !Number.isSafeInteger(n) || n < control.min) {
        return { ok: false, message: `Use a whole number of at least ${control.min}.` };
      }
      return { ok: true, json: String(n) };
    }
    case "switch":
      return { ok: true, json: input === "on" ? "true" : "false" };
    case "text":
    case "choice":
      return { ok: true, json: JSON.stringify(input) };
  }
}

/** The edits a draft makes to the file: only keys whose parsed value differs from the file's, never a locked key. A cleared key has no `value`. */
export function formEdits(curated: SettingValue[], draft: Draft): { edits: SettingEdit[]; errors: Record<string, string> } {
  const edits: SettingEdit[] = [];
  const errors: Record<string, string> = {};
  for (const v of curated) {
    if (v.lock || !v.control || !(v.key in draft)) continue;
    const input = draft[v.key]!;
    const original = draftValue(v);
    if (input === original) continue;
    const parsed = parseField(v.control, input);
    if (!parsed.ok) {
      errors[v.key] = parsed.message;
      continue;
    }
    const before = parseField(v.control, original);
    if (before.ok && before.json === parsed.json) continue;
    edits.push(parsed.json === null ? { key: v.key } : { key: v.key, value: parsed.json });
  }
  return { edits, errors };
}

const label = (value: string) => CHOICE_LABELS[value] ?? value.charAt(0).toUpperCase() + value.slice(1);

/** Not set first, then each control value; a current value outside them comes next, marked as from the file. */
export function choiceOptions(control: Choice, current: string): { value: string; label: string }[] {
  const options = [{ value: "", label: "Not set" }, ...control.values.map((value) => ({ value, label: label(value) }))];
  if (current !== "" && !control.values.includes(current)) options.push({ value: current, label: `${current} (from the file)` });
  return options;
}

export const editSummary = (n: number) => changeSummary(n);
