import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { Appearance } from "../../generated/Appearance";
import type { FoundFile } from "../../generated/FoundFile";
import { getEndpoint } from "../../lib/api/client";
import { settings as settingsApi } from "../../lib/api/settings";
import { applyRootState, DEFAULT_APPEARANCE, rootState } from "./rootState";
import { SettingsReadError, SettingsSync } from "./settingsSync";

type AppearanceContext = {
  appearance: Appearance;
  /** Applies at once; rolls back and sets `error` if the core refuses or can't be reached. */
  save(next: Appearance): Promise<void>;
  error: string | null;
  /** Windows Terminal's settings files, for its font and colour schemes; null until read (or when they can't be). */
  wtFiles: FoundFile[] | null;
  /** The one settings writer, for what is not appearance (the first-Pause warning). Null outside the provider. */
  sync: SettingsSync | null;
};

const Ctx = createContext<AppearanceContext>({ appearance: DEFAULT_APPEARANCE, save: async () => {}, error: null, wtFiles: null, sync: null });

export function useAppearance(): AppearanceContext {
  return useContext(Ctx);
}

const CONTRAST_QUERY = "(prefers-contrast: more)";

/** Loads the Appearance settings once and keeps the root element's theme, accent, contrast and text size in step with them. */
export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [appearance, setAppearance] = useState(DEFAULT_APPEARANCE);
  const [error, setError] = useState<string | null>(null);
  const [wtFiles, setWtFiles] = useState<FoundFile[] | null>(null);
  const [contrastMore, setContrastMore] = useState(() => matchMedia(CONTRAST_QUERY).matches);
  // The last settings the core accepted, to roll back to.
  const saved = useRef(DEFAULT_APPEARANCE);
  const sync = useMemo(() => new SettingsSync(settingsApi, getEndpoint), []);

  useEffect(() => {
    let live = true;
    // If the settings can't be read the defaults stay: the app never waits on them. A later save reads again before it writes.
    sync
      .load()
      .then((loaded) => {
        if (!live) return;
        saved.current = loaded;
        setAppearance(loaded);
      })
      .catch(() => {});
    // Read once, only to show the font and schemes found there; nothing is ever written back.
    getEndpoint()
      .then((ep) => settingsApi.windowsTerminal(ep))
      .then((files) => live && setWtFiles(files))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [sync]);

  useEffect(() => {
    const query = matchMedia(CONTRAST_QUERY);
    const onChange = () => setContrastMore(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    applyRootState(document.documentElement, rootState(appearance, { prefersContrastMore: contrastMore }));
  }, [appearance, contrastMore]);

  const save = useCallback(async (next: Appearance) => {
    setAppearance(next);
    setError(null);
    try {
      const stored = await sync.save(next);
      saved.current = stored;
      setAppearance(stored);
    } catch (e) {
      setAppearance(saved.current);
      if (e instanceof SettingsReadError) setError(e.message);
      else setError(`Couldn't save the appearance settings: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, [sync]);

  const value = useMemo(() => ({ appearance, save, error, wtFiles, sync }), [appearance, save, error, wtFiles, sync]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
