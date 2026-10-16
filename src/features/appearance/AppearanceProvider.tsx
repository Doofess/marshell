import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { Appearance } from "../../generated/Appearance";
import { getEndpoint } from "../../lib/api/client";
import { settings as settingsApi } from "../../lib/api/settings";
import { applyRootState, DEFAULT_APPEARANCE, rootState } from "./rootState";

type AppearanceContext = {
  appearance: Appearance;
  /** Applies at once; rolls back and sets `error` if the core refuses or can't be reached. */
  save(next: Appearance): Promise<void>;
  error: string | null;
};

const Ctx = createContext<AppearanceContext>({ appearance: DEFAULT_APPEARANCE, save: async () => {}, error: null });

export function useAppearance(): AppearanceContext {
  return useContext(Ctx);
}

const CONTRAST_QUERY = "(prefers-contrast: more)";

/** Loads the Appearance settings once and keeps the root element's theme, accent, contrast and text size in step with them. */
export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [appearance, setAppearance] = useState(DEFAULT_APPEARANCE);
  const [error, setError] = useState<string | null>(null);
  const [contrastMore, setContrastMore] = useState(() => matchMedia(CONTRAST_QUERY).matches);
  // The last settings the core accepted, to roll back to.
  const saved = useRef(DEFAULT_APPEARANCE);

  useEffect(() => {
    let live = true;
    // If the settings can't be read the defaults stay: the app never waits on them.
    getEndpoint()
      .then(async (ep) => {
        const s = await settingsApi.get(ep);
        if (!live) return;
        saved.current = s.appearance;
        setAppearance(s.appearance);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, []);

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
      // Asked each time: a restarted core listens on a new endpoint.
      const s = await settingsApi.put(await getEndpoint(), { appearance: next });
      saved.current = s.appearance;
      setAppearance(s.appearance);
    } catch (e) {
      setAppearance(saved.current);
      setError(`Couldn't save the appearance settings: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, []);

  const value = useMemo(() => ({ appearance, save, error }), [appearance, save, error]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
