import type { Appearance } from "../../generated/Appearance";
import type { ImportedTheme } from "../../generated/ImportedTheme";

/** Adds imported themes; one whose id is already taken gets "-2", "-3", and so on, so two imports never share an id. */
export function addImported(a: Appearance, themes: ImportedTheme[]): Appearance {
  const used = new Set(a.imported_themes.map((t) => t.id));
  const added = themes.map((t) => {
    let id = t.id;
    for (let n = 2; used.has(id); n++) id = `${t.id}-${n}`;
    used.add(id);
    return { ...t, id };
  });
  return { ...a, imported_themes: [...a.imported_themes, ...added] };
}

/** Removes an imported theme. When the terminal used it, the terminal goes back to following the app. */
export function removeImported(a: Appearance, id: string): Appearance {
  return {
    ...a,
    imported_themes: a.imported_themes.filter((t) => t.id !== id),
    terminal_theme: a.terminal_theme === id ? "follow-app" : a.terminal_theme,
  };
}
