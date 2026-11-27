import type { Appearance } from "../../generated/Appearance";

/** What a field shows while it is being edited, and why it was refused, if it was. */
export type Draft = { text: string; reason: string | null };

/** The text fields of the dialog. */
export type Field = "font" | "size";

export type Drafts = {
  font: string;
  size: Draft;
};

/** The field text for settings that are saved: the font and the size. */
export function draftsOf(a: Appearance): Drafts {
  return {
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
    font: focused === "font" ? prev.font : fresh.font,
    size: focused === "size" ? prev.size : fresh.size,
  };
}
