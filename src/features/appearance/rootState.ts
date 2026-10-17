import type { Appearance } from "../../generated/Appearance";
import { checkAccent } from "./customAccent";

/** What the app shows before the settings arrive, and when they can't be read. Matches the core's defaults. */
export const DEFAULT_APPEARANCE: Appearance = {
  theme: "follow-system",
  accent: "sand",
  contrast: "follow-system",
  text_size: "default",
  terminal_theme: "follow-app",
  terminal_font: { family: null, size: 13 },
  contrast_protection: true,
  imported_themes: [],
};

export type RootState = {
  attrs: Record<"data-theme" | "data-accent" | "data-contrast" | "data-text-size", string | null>;
  style: Record<"--accent" | "--accent-ink", string | null>;
};

/** The attributes and inline properties the root element carries for these settings. `null` removes one. */
export function rootState(a: Appearance, sys: { prefersContrastMore: boolean }): RootState {
  let accent: string | null = a.accent;
  let style: RootState["style"] = { "--accent": null, "--accent-ink": null };
  if (a.accent.startsWith("#")) {
    const check = checkAccent(a.accent);
    // A custom accent that fails the check (edited by hand in settings.json) falls back to the default preset.
    if (check.ok) style = { "--accent": check.hex, "--accent-ink": check.ink };
    accent = check.ok ? "custom" : DEFAULT_APPEARANCE.accent;
  }
  const contrastMore = a.contrast === "on" || (a.contrast === "follow-system" && sys.prefersContrastMore);
  return {
    attrs: {
      "data-theme": a.theme === "follow-system" ? null : a.theme,
      "data-accent": accent,
      "data-contrast": contrastMore ? "more" : null,
      "data-text-size": a.text_size === "default" ? null : a.text_size,
    },
    style,
  };
}

/** Writes a root state onto an element (the document root in the app). */
export function applyRootState(el: HTMLElement, s: RootState): void {
  for (const [name, value] of Object.entries(s.attrs)) {
    if (value === null) el.removeAttribute(name);
    else el.setAttribute(name, value);
  }
  for (const [name, value] of Object.entries(s.style)) {
    if (value === null) el.style.removeProperty(name);
    else el.style.setProperty(name, value);
  }
}
