import { useEffect, useEffectEvent } from "react";
import type { Os } from "../../lib/keymap";
import { subscribeNav } from "../sessions/useNavigation";
import { appClaims, claimedChords } from "./claimedChords";
import { browser, onBrowser } from "./browserApi";
import { pageCovered, subscribeCover } from "./pageCover";
import { handBack, keyForCode, onDragArea, quietly, tabNeighbour } from "./pageFocus";
import { dispatchPage } from "./pageStore";

const TABBABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
const ignore = () => {};

/** The slot a page's key belongs to (BrowserTab puts the key on it), when that tab is drawn. */
function slotOf(key: string): HTMLElement | null {
  for (const el of document.querySelectorAll<HTMLElement>("[data-page-key]")) if (el.dataset.pageKey === key) return el;
  return null;
}

/** The app's tab order, as Tab walks it: shown, not inert, not taken out with tabindex -1. */
function tabbables(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>(TABBABLE)].filter((e) => e.tabIndex >= 0 && e.getClientRects().length > 0 && !e.closest("[inert]"));
}

/** The URL bar's address box in the tab that owns this slot. */
function addressBox(slot: HTMLElement): HTMLInputElement | null {
  return slot.closest(".browser-tab")?.querySelector<HTMLInputElement>('input[aria-label="Address"]') ?? null;
}

type Options = {
  os: Os | undefined;
  /** Esc in a page: the keyboard goes back to the terminal, as Esc in a file tab does. */
  focusTerminal(): void;
};

/**
 * The app's side of a page's keyboard (S12; 6q-0 decision 9). Sends the shell the chords the app claims, and acts on what the shell
 * sends back: a claimed key runs through the app's own keymap as a synthetic key-down on the page's slot, Tab off either end of
 * the page lands on the app's element on that side, and the keys go back to the page where it had them (after a key that left it
 * there, after a cover goes, after the window comes back).
 */
export function useBrowserKeys({ os, focusTerminal }: Options): void {
  const terminal = useEffectEvent(focusTerminal);

  // The claimed list: sent on mount, and again whenever it would differ (the navigation chords count once navigation has started).
  useEffect(() => {
    if (os === undefined) return;
    let sent = "";
    const send = () => {
      const chords = claimedChords(appClaims(os, "terminal"));
      const key = JSON.stringify(chords);
      if (key === sent) return;
      sent = key;
      browser.keys(chords).catch(ignore);
    };
    send();
    return subscribeNav(send);
  }, [os]);

  useEffect(() => {
    // The page whose turn at the keyboard this is: set when it takes the keys, and kept while the app borrows them for a key; ended
    // by a press in the app, by Tab leaving the page, by Esc and by Ctrl+L.
    let owner: string | null = null;
    let pointerAt = -Infinity;
    // Whether the page had the keys when the cover came; a cover is drawn after the key that opened it, so this is read at the cover.
    let heldAtCover: string | null = null;
    let frame = 0;

    const stops = [
      onBrowser("focus", ({ key, focused }) => {
        if (focused) owner = key;
      }),
      onBrowser("focus-out", ({ key, direction }) => {
        owner = null;
        const slot = slotOf(key);
        if (!slot) return;
        const target = tabNeighbour(tabbables(), slot, direction);
        if (target) quietly(() => target.focus());
      }),
      onBrowser("key", ({ key, code, ctrl, shift, alt, repeat }) => {
        const slot = slotOf(key);
        // The shell has moved the keyboard to the app; say so before the app's handlers look at where it is.
        dispatchPage({ type: "focus", key, focused: false });
        if (!slot) return;
        if (code === "Escape" && !ctrl && !shift && !alt) {
          owner = null;
          terminal();
          return;
        }
        if (code === "KeyL" && ctrl && !shift && !alt) {
          owner = null;
          const box = addressBox(slot);
          if (box) {
            box.focus();
            box.select();
          }
          return;
        }
        quietly(() => slot.focus({ preventScroll: true }));
        slot.dispatchEvent(new KeyboardEvent("keydown", { code, key: keyForCode(code, shift), ctrlKey: ctrl, shiftKey: shift, altKey: alt, repeat, bubbles: true, cancelable: true }));
        // The key was the app's, and the page's turn comes back with the next frame unless the key moved focus or covered the page.
        cancelAnimationFrame(frame);
        frame = requestAnimationFrame(() => {
          if (document.activeElement === slot && !pageCovered()) browser.focus(key, "programmatic").catch(ignore);
        });
      }),
    ];

    // After a cover goes: the page gets the keys back if it had them when the cover came, and focus is where the cover left it
    // (the slot, or on nothing).
    const stopCover = subscribeCover(() => {
      if (pageCovered()) {
        heldAtCover = owner;
        return;
      }
      const key = heldAtCover;
      heldAtCover = null;
      if (key === null) return;
      requestAnimationFrame(() => {
        const slot = slotOf(key);
        if (!slot || pageCovered()) return;
        const on = document.activeElement;
        if (on !== slot && on !== document.body && on !== null) return;
        if (on !== slot) quietly(() => slot.focus({ preventScroll: true }));
        browser.focus(key, "programmatic").catch(ignore);
      });
    });

    // The window comes back to the front (Alt+Tab, a click on the title bar): wry has moved the keys to the app's webview, and the
    // page gets them back when it had them and nothing in the app was pressed since (S12 3b, finding 4).
    const onPointer = (e: PointerEvent) => {
      if (e.target instanceof Element && onDragArea(e.target)) return;
      pointerAt = performance.now();
      owner = null;
    };
    const onWindowFocus = () => {
      const at = performance.now();
      const key = owner;
      if (key === null) return;
      setTimeout(() => {
        if (key !== owner) return;
        if (handBack({ pageHadKeys: owner !== null, pointerSince: pointerAt > at, covered: pageCovered(), docHasFocus: document.hasFocus() })) browser.focus(key, "programmatic").catch(ignore);
      }, 100);
    };
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("focus", onWindowFocus);

    return () => {
      stops.forEach((stop) => stop());
      stopCover();
      cancelAnimationFrame(frame);
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("focus", onWindowFocus);
    };
  }, []);
}
