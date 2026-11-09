// Spike 6q-0 (never merged): the browser pane's state, and `window.__spike` for CDP to drive it in dev.
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { inTauri } from "../../lib/window/appWindow";

export type SpikeControls = {
  open(url: string): void;
  close(): void;
  setInset(px: number): void;
  /** Who moves the page when the window resizes; `true` and `false` are the older scripts' anchor and follow. */
  setMode(mode: SpikeMode | boolean): void;
  keyTest(on: boolean): void;
  /** The overlay freeze (Task 5), on by default; off shows tauri#15682, the page drawn over the app's overlays. */
  setFreeze(on: boolean): void;
  /** Changes inside the window (the divider, the sidebar): the page moves this many painted frames after the app measured
   * it, 0 at once. */
  setSync(frames: number): void;
};

/** The in-window delay `setSync` sets; BrowserSpike reads it on every report. */
export const paintSync = { frames: 0 };

declare global {
  interface Window {
    __spike?: SpikeControls;
  }
}

export type SpikeMode = "follow" | "anchor" | "predict" | "rule";

export type SpikeLog = (event: string, data?: Record<string, unknown>) => void;

/** Every front-side spike event goes to the same JSONL file as Rust's. */
export const spikeLog: SpikeLog = (event, data = {}) => {
  void invoke("spike_log", { event, data: { ...data, perf: Math.round(performance.now()) } }).catch(() => {});
};

export type BrowserSpikeModel = {
  active: boolean;
  visible: boolean;
  /** The URL the pane opens or goes to next; `seq` changes on every `open`, so the same URL can be asked for twice. */
  request: { url: string; seq: number } | null;
  inset: number;
  keyTest: boolean;
  freeze: boolean;
  /** Whether the child exists, so a pane shown again shows it rather than creating a second one. */
  created: React.MutableRefObject<boolean>;
  toggle(): void;
};

export function useBrowserSpike(): BrowserSpikeModel {
  const [active, setActive] = useState(false);
  const [visible, setVisible] = useState(true);
  const [request, setRequest] = useState<{ url: string; seq: number } | null>(null);
  const [inset, setInset] = useState(0);
  const [keyTest, setKeyTest] = useState(false);
  const [freeze, setFreeze] = useState(true);
  const created = useRef(false);

  useEffect(() => {
    if (!import.meta.env.DEV || !inTauri()) return;
    window.__spike = {
      open(url) {
        setActive(true);
        setVisible(true);
        setRequest((r) => ({ url, seq: (r?.seq ?? 0) + 1 }));
      },
      close() {
        void invoke("spike_browser_close").catch(() => {});
        created.current = false;
        setActive(false);
        setRequest(null);
      },
      setInset: (px) => setInset(px),
      setMode: (mode) => void invoke("spike_browser_mode", { mode: mode === true ? "anchor" : mode === false ? "follow" : mode }),
      keyTest: (on) => setKeyTest(on),
      setFreeze: (on) => setFreeze(on),
      setSync(frames) {
        paintSync.frames = Math.max(0, Math.round(frames));
        spikeLog("sync-set", { frames: paintSync.frames });
      },
    };
    return () => {
      delete window.__spike;
    };
  }, []);

  const toggle = useCallback(() => {
    setVisible((v) => {
      spikeLog(v ? "pane-hidden" : "pane-shown");
      return !v;
    });
  }, []);

  return useMemo(() => ({ active, visible, request, inset, keyTest, freeze, created, toggle }), [active, visible, request, inset, keyTest, freeze, toggle]);
}
