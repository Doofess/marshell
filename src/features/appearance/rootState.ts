import type { Appearance } from "../../generated/Appearance";

/** What the app shows before the settings arrive, and when they can't be read. Matches the core's defaults. */
export const DEFAULT_APPEARANCE: Appearance = {
  theme: "follow-system",
  contrast: "follow-system",
  text_size: "default",
  terminal_theme: "follow-app",
  terminal_font: { family: null, size: 13 },
  contrast_protection: true,
  imported_themes: [],
};

export type RootState = {
  attrs: Record<"data-theme" | "data-contrast" | "data-text-size", string | null>;
};

/** Whether High contrast is on: by the setting, or by the system asking for more contrast while the setting follows it. */
export function contrastMoreOf(contrast: Appearance["contrast"], prefersContrastMore: boolean): boolean {
  return contrast === "on" || (contrast === "follow-system" && prefersContrastMore);
}

/** The attributes the root element carries for these settings. `null` removes one. */
export function rootState(a: Appearance, sys: { prefersContrastMore: boolean }): RootState {
  const contrastMore = contrastMoreOf(a.contrast, sys.prefersContrastMore);
  return {
    attrs: {
      "data-theme": a.theme === "follow-system" ? null : a.theme,
      "data-contrast": contrastMore ? "more" : null,
      "data-text-size": a.text_size === "default" ? null : a.text_size,
    },
  };
}

/** Writes a root state onto an element (the document root in the app). */
export function applyRootState(el: HTMLElement, s: RootState): void {
  for (const [name, value] of Object.entries(s.attrs)) {
    if (value === null) el.removeAttribute(name);
    else el.setAttribute(name, value);
  }
}
