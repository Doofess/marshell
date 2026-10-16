import { useEffect, useEffectEvent, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Page } from "../../components/Page/Page";
import { Switch } from "../../components/Switch/Switch";
import type { Appearance } from "../../generated/Appearance";
import type { FoundFile } from "../../generated/FoundFile";
import type { ImportedTheme } from "../../generated/ImportedTheme";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { BUNDLED_FONT, fontStack, resolveTerminalFont } from "../terminal/terminalFont";
import { appScheme, resolveTerminalTheme, type Scheme } from "../terminal/terminalTheme";
import { addImported, removeImported } from "./appearanceEdits";
import { useAppearance } from "./AppearanceProvider";
import { checkAccent } from "./customAccent";
import { importThemeFile, MAX_THEME_FILE } from "./import/importTheme";
import { parseWtSchemes } from "./import/windowsTerminal";
import { terminalThemeOf, wtFont } from "./terminalSettings";
import "../settings/Settings.css";
import "./AppearanceDialog.css";

/** Where a failed save is shown: next to the part that was changed. */
export type AppearanceSection = "theme" | "accent" | "terminal" | "imported";

type Draft = { text: string; reason: string | null };

export type AppearanceViewState = {
  appearance: Appearance;
  /** The scheme the app shows now, for the terminal preview. */
  app: Scheme;
  wtFiles: FoundFile[] | null;
  error: { at: AppearanceSection; text: string } | null;
  accent: Draft;
  font: string;
  size: Draft;
  importError: string | null;
};

export type AppearanceHandlers = {
  save(next: Appearance, at: AppearanceSection): void;
  onAccentText(text: string): void;
  /** Saves the custom accent if it passes the check; otherwise the reason shows and the accent stays. */
  commitAccent(): void;
  onFontText(text: string): void;
  commitFont(): void;
  onSizeText(text: string): void;
  commitSize(): void;
  importFile(file: File): void;
  onClose?(): void;
};

const ACCENTS = [
  ["amber", "Amber"],
  ["blue", "Blue"],
  ["indigo", "Indigo"],
  ["violet", "Violet"],
  ["magenta", "Magenta"],
  ["cyan", "Cyan"],
  ["teal", "Teal"],
  ["slate", "Slate"],
] as const;

const ANSI_KEYS = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite"] as const;

const FONT_SOURCE = { yours: "Yours", "windows-terminal": "From Windows Terminal", bundled: "Bundled" } as const;

const onEnter = (commit: () => void) => (e: KeyboardEvent) => {
  if (e.key === "Enter") commit();
};

/** The schemes in Windows Terminal's settings files; a file that doesn't parse adds none. */
function wtSchemes(files: FoundFile[]): ImportedTheme[] {
  return files.flatMap((f) => {
    try {
      return parseWtSchemes(f.text);
    } catch {
      return [];
    }
  });
}

function ErrorAt({ at, error }: { at: AppearanceSection; error: AppearanceViewState["error"] }) {
  return error?.at === at ? (
    <p className="appearance__error">{error.text}</p>
  ) : null;
}

/** A native radio group drawn as chips; the chosen one carries the selected mark. */
function Choice<T extends string>({ legend, help, name, value, options, onPick }: { legend: string; help?: string; name: string; value: T; options: readonly (readonly [T, string])[]; onPick: (v: T) => void }) {
  const helpId = useId();
  return (
    <fieldset className="appearance__group" aria-describedby={help ? helpId : undefined}>
      <legend className="field__label">{legend}</legend>
      {help && (
        <p id={helpId} className="field__help">
          {help}
        </p>
      )}
      <div className="appearance__chips">
        {options.map(([v, label]) => (
          <label key={v} className="appearance__chip is-selected-mark" data-selected={v === value ? "true" : undefined}>
            <input type="radio" name={name} value={v} checked={v === value} onChange={() => onPick(v)} />
            <span>{label}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section className="appearance__section" aria-labelledby={id}>
      <h3 id={id} className="appearance__heading">
        {title}
      </h3>
      {children}
    </section>
  );
}

/** The Appearance settings: app theme, accent, contrast, text size, the terminal's theme and font, and imported themes. */
export function AppearanceView({ state, handlers }: { state: AppearanceViewState; handlers: AppearanceHandlers }) {
  const { appearance: a, error } = state;
  const ids = { accentHelp: useId(), accentWhy: useId(), themeHelp: useId(), font: useId(), fontHelp: useId(), size: useId(), sizeWhy: useId(), file: useId() };
  const custom = a.accent.startsWith("#");
  const preview = resolveTerminalTheme({ ...terminalThemeOf(a, null), app: state.app }).theme;
  const font = resolveTerminalFont(a.terminal_font, wtFont(state.wtFiles ?? []));
  const found = useMemo(() => wtSchemes(state.wtFiles ?? []), [state.wtFiles]);
  const have = new Set(a.imported_themes.map((t) => t.id));
  const set = (patch: Partial<Appearance>, at: AppearanceSection) => handlers.save({ ...a, ...patch }, at);

  const close = handlers.onClose && (
    <button type="button" className="settings__btn" onClick={handlers.onClose}>
      Close
    </button>
  );

  return (
    <Page title="Appearance" subtitle="How Marshell and its terminals look." actions={close}>
      <div className="appearance">
        <Section title="App">
          <Choice legend="Theme" name="theme" value={a.theme} options={[["follow-system", "Follow system"], ["dark", "Dark"], ["light", "Light"]]} onPick={(theme) => set({ theme }, "theme")} />
          <Choice legend="High contrast" help="Follow system turns this on when Windows asks for more contrast." name="contrast" value={a.contrast} options={[["follow-system", "Follow system"], ["on", "On"], ["off", "Off"]]} onPick={(contrast) => set({ contrast }, "theme")} />
          <Choice legend="Text size" help="Changes the size of Marshell's text. The terminal font size is set below." name="text-size" value={a.text_size} options={[["default", "Default"], ["large", "Large"], ["larger", "Larger"]]} onPick={(text_size) => set({ text_size }, "theme")} />
          <ErrorAt at="theme" error={error} />
        </Section>

        <Section title="Accent">
          <fieldset className="appearance__group" aria-describedby={ids.accentHelp}>
            <legend className="field__label">Accent colour</legend>
            <p id={ids.accentHelp} className="field__help">
              Sessions use their agent&apos;s colour. Your accent shows everywhere else.
            </p>
            <div className="appearance__chips">
              {ACCENTS.map(([v, label]) => (
                <label key={v} className="appearance__chip is-selected-mark" data-selected={a.accent === v ? "true" : undefined}>
                  <input type="radio" name="accent" value={v} checked={a.accent === v} onChange={() => set({ accent: v }, "accent")} />
                  <span className="appearance__swatch" data-accent={v} aria-hidden="true" />
                  <span>{label}</span>
                </label>
              ))}
              <label className="appearance__chip is-selected-mark" data-selected={custom ? "true" : undefined}>
                <input type="radio" name="accent" value="custom" checked={custom} onChange={handlers.commitAccent} />
                <span className="appearance__swatch appearance__swatch--custom" style={custom ? { background: a.accent } : undefined} aria-hidden="true" />
                <span>Custom</span>
              </label>
            </div>
            <label className="appearance__inline">
              <span>Custom colour</span>
              <input
                type="text"
                className="field__control appearance__hex"
                spellCheck={false}
                placeholder="#3a7bd5"
                value={state.accent.text}
                aria-invalid={state.accent.reason ? true : undefined}
                aria-describedby={state.accent.reason ? ids.accentWhy : undefined}
                onChange={(e) => handlers.onAccentText(e.target.value)}
                onBlur={handlers.commitAccent}
                onKeyDown={onEnter(handlers.commitAccent)}
              />
            </label>
            {state.accent.reason && (
              <p id={ids.accentWhy} className="field__why">
                {state.accent.reason}
              </p>
            )}
          </fieldset>
          <ErrorAt at="accent" error={error} />
        </Section>

        <Section title="Terminal">
          <div className="appearance__group">
            <label className="field__label" htmlFor={`${ids.themeHelp}-select`}>
              Terminal theme
            </label>
            <p id={ids.themeHelp} className="field__help">
              Applies to every terminal, unless its shell profile picks its own. A running program may not notice the change until it redraws or restarts.
            </p>
            <select id={`${ids.themeHelp}-select`} className="field__control" aria-describedby={ids.themeHelp} value={a.terminal_theme} onChange={(e) => set({ terminal_theme: e.target.value }, "terminal")}>
              <option value="follow-app">Follow the app</option>
              <option value="dark">Dark</option>
              <option value="light">Light</option>
              {a.imported_themes.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
            <div className="appearance__preview" style={{ background: preview.background, color: preview.foreground, fontFamily: fontStack(font.family) }} role="img" aria-label="Preview of the terminal theme's colours">
              <span className="appearance__preview-text" aria-hidden="true">
                ~/marshell $ ls
              </span>
              <span className="appearance__ansi" aria-hidden="true">
                {ANSI_KEYS.map((k) => (
                  <span key={k} className="appearance__ansi-cell" style={{ background: preview[k] }} />
                ))}
              </span>
            </div>
          </div>

          <div className="appearance__group">
            <label className="field__label" htmlFor={ids.font}>
              Terminal font
            </label>
            <p id={ids.fontHelp} className="field__help">
              {FONT_SOURCE[font.source]}: {font.family}. Leave the field empty to use {state.wtFiles?.length ? "Windows Terminal's font, or " : ""}
              {BUNDLED_FONT}.
            </p>
            <input id={ids.font} type="text" className="field__control" spellCheck={false} placeholder="Automatic" value={state.font} aria-describedby={ids.fontHelp} onChange={(e) => handlers.onFontText(e.target.value)} onBlur={handlers.commitFont} onKeyDown={onEnter(handlers.commitFont)} />
          </div>

          <div className="appearance__group">
            <label className="field__label" htmlFor={ids.size}>
              Terminal font size
            </label>
            <input
              id={ids.size}
              type="number"
              min={8}
              max={32}
              step={1}
              className="field__control appearance__size"
              value={state.size.text}
              aria-invalid={state.size.reason ? true : undefined}
              aria-describedby={state.size.reason ? ids.sizeWhy : undefined}
              onChange={(e) => handlers.onSizeText(e.target.value)}
              onBlur={handlers.commitSize}
              onKeyDown={onEnter(handlers.commitSize)}
            />
            {state.size.reason && (
              <p id={ids.sizeWhy} className="field__why">
                {state.size.reason}
              </p>
            )}
          </div>

          <div className="appearance__group">
            <Switch label="Contrast protection" checked={a.contrast_protection} onChange={(contrast_protection) => set({ contrast_protection }, "terminal")} />
            <p className="field__help">Lifts dim text so it stays readable on the terminal&apos;s background.</p>
          </div>
          <ErrorAt at="terminal" error={error} />
        </Section>

        <Section title="Imported themes">
          <div className="appearance__group">
            <label className="field__label" htmlFor={ids.file}>
              Import a theme file
            </label>
            <p className="field__help">A Windows Terminal settings or scheme file, an iTerm2 .itermcolors file, or a VS Code colour theme.</p>
            <input
              id={ids.file}
              type="file"
              accept=".json,.itermcolors"
              className="appearance__file"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) handlers.importFile(file);
                e.target.value = "";
              }}
            />
            {state.importError && (
              <p className="field__why">
                {state.importError}
              </p>
            )}
          </div>

          {a.imported_themes.length > 0 && (
            <ul className="appearance__list" aria-label="Your imported themes">
              {a.imported_themes.map((t) => (
                <li key={t.id} className="appearance__row">
                  <ThemeDots theme={t} />
                  <span className="appearance__row-name">{t.name}</span>
                  <button type="button" className="settings__btn" aria-label={`Remove ${t.name}`} onClick={() => handlers.save(removeImported(a, t.id), "imported")}>
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}

          <h4 className="field__label">In your Windows Terminal settings</h4>
          {found.length === 0 ? (
            <p className="field__help">{state.wtFiles?.length ? "Your Windows Terminal settings have no colour schemes." : "No Windows Terminal settings found."}</p>
          ) : (
            <ul className="appearance__list" aria-label="Schemes in your Windows Terminal settings">
              {found.map((t, i) => (
                <li key={`${t.id}-${i}`} className="appearance__row">
                  <ThemeDots theme={t} />
                  <span className="appearance__row-name">{t.name}</span>
                  <button type="button" className="settings__btn" aria-label={have.has(t.id) ? `${t.name} added` : `Add ${t.name}`} disabled={have.has(t.id)} onClick={() => handlers.save(addImported(a, [t]), "imported")}>
                    {have.has(t.id) ? "Added" : "Add"}
                  </button>
                </li>
              ))}
            </ul>
          )}
          <ErrorAt at="imported" error={error} />
        </Section>
      </div>
    </Page>
  );
}

/** A theme's background with its first eight colours, as a small sample next to its name. */
function ThemeDots({ theme }: { theme: ImportedTheme }) {
  return (
    <span className="appearance__dots" style={{ background: theme.colors.background }} aria-hidden="true">
      {theme.colors.ansi.slice(1, 7).map((c, i) => (
        <span key={i} style={{ background: c }} />
      ))}
    </span>
  );
}

const sizeReason = "Enter a whole number from 8 to 32.";

/** The Appearance dialog over the live window. Each change saves at once; Escape or Close closes it. */
export function AppearanceDialog({ onClose, onNotice }: { onClose: () => void; onNotice?: (text: string) => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  useDialogFocus(dialog, dialog, onClose);
  const { appearance, save, error, wtFiles } = useAppearance();
  const [at, setAt] = useState<AppearanceSection>("theme");
  const [accent, setAccent] = useState<Draft>({ text: appearance.accent.startsWith("#") ? appearance.accent : "", reason: null });
  const [font, setFont] = useState(appearance.terminal_font.family ?? "");
  const [size, setSize] = useState<Draft>({ text: String(appearance.terminal_font.size), reason: null });
  const [importError, setImportError] = useState<string | null>(null);
  // The window has one live region (its announcer): failures are said there and shown in words beside their section.
  const say = useEffectEvent((text: string) => onNotice?.(text));
  useEffect(() => {
    if (error) say(error);
  }, [error]);
  useEffect(() => {
    if (importError) say(importError);
  }, [importError]);

  const handlers: AppearanceHandlers = {
    save(next, section) {
      setAt(section);
      void save(next);
    },
    onAccentText: (text) => setAccent({ text, reason: null }),
    commitAccent() {
      const check = checkAccent(accent.text);
      if (!check.ok) return setAccent({ ...accent, reason: check.reason });
      setAccent({ text: check.hex, reason: null });
      if (check.hex !== appearance.accent) handlers.save({ ...appearance, accent: check.hex }, "accent");
    },
    onFontText: setFont,
    commitFont() {
      const family = font.trim() || null;
      if (family !== appearance.terminal_font.family) handlers.save({ ...appearance, terminal_font: { ...appearance.terminal_font, family } }, "terminal");
    },
    onSizeText: (text) => setSize({ text, reason: null }),
    commitSize() {
      const n = Number(size.text);
      if (!/^\d+$/.test(size.text.trim()) || n < 8 || n > 32) return setSize({ ...size, reason: sizeReason });
      if (n !== appearance.terminal_font.size) handlers.save({ ...appearance, terminal_font: { ...appearance.terminal_font, size: n } }, "terminal");
    },
    importFile(file) {
      setImportError(null);
      if (file.size > MAX_THEME_FILE) return setImportError(`${file.name} is too large to be a theme file.`);
      file
        .text()
        .then((text) => {
          const result = importThemeFile(file.name, text);
          if (!result.ok) return setImportError(result.reason);
          handlers.save(addImported(appearance, result.themes), "imported");
          onNotice?.(result.themes.length === 1 ? `Imported ${result.themes[0]!.name}.` : `Imported ${result.themes.length} themes.`);
        })
        .catch(() => setImportError(`Couldn't read ${file.name}.`));
    },
    onClose,
  };

  const state: AppearanceViewState = { appearance, app: appearance.theme === "follow-system" ? appScheme() : appearance.theme, wtFiles, error: error ? { at, text: error } : null, accent, font, size, importError };
  return (
    <div ref={dialog} className="appearance-dialog" role="dialog" aria-modal="true" aria-label="Appearance" tabIndex={-1}>
      <AppearanceView state={state} handlers={handlers} />
    </div>
  );
}
