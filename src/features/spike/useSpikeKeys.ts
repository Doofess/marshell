// Spike 6q-0 (never merged): what the app does with a key the page handed over (6q decision 4), and the key test's
// reset between presses.
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { call } from "./BrowserSpike";
import { spikeLog, type BrowserSpikeModel } from "./useBrowserSpike";

type KeyPayload = { code: string; ctrl: boolean; shift: boolean; alt: boolean; repeat: boolean; n: number };

function keyFor(code: string, shift: boolean): string {
  if (code.startsWith("Key")) return shift ? code.slice(3) : code.slice(3).toLowerCase();
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

type Options = {
  spike: BrowserSpikeModel;
  overlay: string | null;
  focusTerminal(): void;
  closeOverlay(): void;
};

export function useSpikeKeys({ spike, overlay, focusTerminal, closeOverlay }: Options) {
  const live = useRef({ spike, focusTerminal, closeOverlay });
  live.current = { spike, focusTerminal, closeOverlay };

  // One action per forwarded key; in the key test it is undone 400 ms later and the page takes the keys again.
  const acted = useRef((what: string) => {
    spikeLog("action", { what });
    if (!live.current.spike.keyTest) return;
    setTimeout(() => {
      live.current.closeOverlay();
      if (!live.current.spike.visible) live.current.spike.toggle();
      setTimeout(() => {
        void call("spike_browser_focus", { reason: "programmatic" }).catch(() => {});
        spikeLog("reset");
      }, 150);
    }, 400);
  });

  useEffect(() => {
    if (!spike.active) return;
    const off = listen<KeyPayload>("spike://key", ({ payload: k }) => {
      spikeLog("key-received", k);
      if (k.code === "KeyL" && k.ctrl && !k.shift) {
        const box = document.querySelector<HTMLInputElement>(".spike-browser__bar input");
        box?.focus();
        box?.select();
        if (document.activeElement === box) acted.current("address-focused");
        return;
      }
      if (k.code === "Escape") {
        live.current.focusTerminal();
        setTimeout(() => {
          if (document.activeElement?.closest(".xterm")) acted.current("terminal-focused");
          else spikeLog("action-missing", { what: "terminal-focused", active: document.activeElement?.className ?? null });
        }, 50);
        return;
      }
      const target = document.activeElement ?? document.body;
      target.dispatchEvent(
        new KeyboardEvent("keydown", { code: k.code, key: keyFor(k.code, k.shift), ctrlKey: k.ctrl, shiftKey: k.shift, altKey: k.alt, repeat: k.repeat, bubbles: true, cancelable: true }),
      );
    });
    return () => void off.then((f) => f());
  }, [spike.active]);

  useEffect(() => {
    if (!spike.active) return;
    spikeLog("overlay", { overlay });
    if (overlay === "palette") acted.current("palette-open");
  }, [overlay, spike.active]);

  const firstVisible = useRef(true);
  useEffect(() => {
    if (firstVisible.current) {
      firstVisible.current = false;
      return;
    }
    // Only hiding is a key's action; showing again is the reset's own doing.
    if (spike.active && !spike.visible) acted.current("pane-hidden");
  }, [spike.visible, spike.active]);
}
