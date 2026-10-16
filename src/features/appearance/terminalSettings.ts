import type { Appearance } from "../../generated/Appearance";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { FoundFile } from "../../generated/FoundFile";
import type { Profile } from "../../generated/Profile";
import type { TerminalThemeProp } from "../terminal/TerminalView";
import { resolveTerminalTheme, type Scheme } from "../terminal/terminalTheme";
import { wtDefaultFont } from "./import/windowsTerminal";

/** What a terminal needs from the Appearance settings to pick its theme, with its profile's override (absent: none). */
export function terminalThemeOf(a: Appearance, override: string | null | undefined): TerminalThemeProp {
  return { setting: a.terminal_theme, override: override ?? null, imported: a.imported_themes, protection: a.contrast_protection };
}

/** The request with the scheme its session's terminal will show, so the core can set COLORFGBG before the program starts. */
export function withColorScheme(req: CreateSessionRequest, profiles: Profile[], a: Appearance, app: Scheme): CreateSessionRequest {
  const override = profiles.find((p) => p.id === req.profile_id)?.terminal_theme;
  return { ...req, color_scheme: resolveTerminalTheme({ ...terminalThemeOf(a, override), app }).scheme };
}

/** The default profile font of the first Windows Terminal settings file that names one; a file that doesn't parse is skipped. */
export function wtFont(files: FoundFile[]): { family?: string; size?: number } | null {
  for (const f of files) {
    try {
      const font = wtDefaultFont(f.text);
      if (font.family) return font;
    } catch {
      // Not valid JSON: try the next file.
    }
  }
  return null;
}
