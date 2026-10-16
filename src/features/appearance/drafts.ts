import type { Appearance } from "../../generated/Appearance";
import { checkAccent } from "./customAccent";

/** What a field shows while it is being edited, and why it was refused, if it was. */
export type Draft = { text: string; reason: string | null };

/** The text fields of the dialog. */
export type Field = "accent" | "font" | "size";

export type Drafts = {
  accent: Draft;
  /** The custom accent text as it was last filled in or committed: leaving the field with that text changes nothing. */
  accentBase: string;
  font: string;
  size: Draft;
};

/** The field text for settings that are saved: the custom accent (empty when a preset is used), the font and the size. */
export function draftsOf(a: Appearance): Drafts {
  const accent = a.accent.startsWith("#") ? a.accent : "";
  return {
    accent: { text: accent, reason: null },
    accentBase: accent,
    font: a.terminal_font.family ?? "",
    size: { text: String(a.terminal_font.size), reason: null },
  };
}

/**
 * The field text after the settings changed (for one, they finished loading). Every field takes the saved value except
 * the one being typed in, so opening the dialog before the settings arrived never saves the empty defaults back.
 */
export function refill(prev: Drafts, a: Appearance, focused: Field | null): Drafts {
  const fresh = draftsOf(a);
  return {
    accent: focused === "accent" ? prev.accent : fresh.accent,
    accentBase: focused === "accent" ? prev.accentBase : fresh.accentBase,
    font: focused === "font" ? prev.font : fresh.font,
    size: focused === "size" ? prev.size : fresh.size,
  };
}

/** Checks the custom accent text. `hex` is the colour to save, or null when the text was refused (the reason is in the drafts). */
export function commitAccent(d: Drafts): { drafts: Drafts; hex: string | null } {
  const check = checkAccent(d.accent.text);
  if (!check.ok) return { drafts: { ...d, accent: { ...d.accent, reason: check.reason } }, hex: null };
  return { drafts: { ...d, accent: { text: check.hex, reason: null }, accentBase: check.hex }, hex: check.hex };
}

/** Leaving the custom accent field. Only text that changed is checked, and empty text is left alone: tabbing past is not a choice. */
export function blurAccent(d: Drafts): { drafts: Drafts; hex: string | null } {
  if (d.accent.text === d.accentBase || d.accent.text.trim() === "") return { drafts: d, hex: null };
  return commitAccent(d);
}

/** Choosing one of the preset accents empties the custom colour field. */
export function pickPreset(d: Drafts): Drafts {
  return { ...d, accent: { text: "", reason: null }, accentBase: "" };
}
