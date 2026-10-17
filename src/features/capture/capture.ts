import { invoke } from "@tauri-apps/api/core";
import { dropDelivery } from "../drop/dropTarget";
import type { RowModel } from "../sidebar/types";

/** How long the window must have had focus back, with no image on the clipboard, before the capture counts as cancelled. */
export const FOCUS_CANCEL_MS = 1500;

export const CAPTURE_WAIT_NOTICE = "Select a region of the screen. Esc in the snipping tool cancels.";
export const CAPTURE_CANCELLED_NOTICE = "The capture was cancelled.";

/** True once the window has had focus back for 1500 ms. Esc in the snipping overlay hands the focus back and nothing else follows. */
export const cancelAfterFocus = (focusedAt: number, now: number): boolean => now - focusedAt >= FOCUS_CANCEL_MS;

/** Opens the snipping tool and waits. True: an image arrived on the clipboard. False: cancelled, or no image within 60 s. Rejects with a sentence. */
export const captureScreenRegion = (): Promise<boolean> => invoke<boolean>("capture_screen_region");

/** Ends the running capture early. With none running it does nothing. */
export const cancelCapture = (): Promise<void> => invoke<void>("cancel_capture");

/** The first image on the clipboard, or null when it holds none. */
export async function readClipboardImage(): Promise<Blob | null> {
  for (const item of await navigator.clipboard.read()) {
    const type = item.types.find((t) => t.startsWith("image/"));
    if (type !== undefined) return item.getType(type);
  }
  return null;
}

/** A failure as a sentence: Tauri rejects with the command's own text, the browser with an Error. */
export function captureFailure(e: unknown): string {
  if (typeof e === "string" && e.trim() !== "") return e;
  return e instanceof Error && e.message.trim() !== "" ? e.message : "Something went wrong.";
}

/** Why a capture can't start for the focused session, checked before the snipping tool opens so no snip is wasted; null when it can. */
export function captureRefusal(focusedId: string | null | undefined, rows: readonly RowModel[]): string | null {
  const delivery = focusedId ? dropDelivery(rows.find((r) => r.id === focusedId) ?? null) : null;
  if (!delivery) return "No session to add it to.";
  return delivery.refused ?? null;
}

type Target = Pick<EventTarget, "addEventListener" | "removeEventListener">;

/**
 * Calls `onCancel` once the window has had focus back for 1500 ms (the person pressed Esc in the snipping overlay). Losing the focus
 * again before then starts the count over. Returns the stop function, which removes the listeners and the pending timer; the caller
 * runs it when the capture ends, however it ends.
 */
export function watchFocusCancel(onCancel: () => void, target: Target = window, now: () => number = Date.now): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const clear = () => {
    clearTimeout(timer);
    timer = undefined;
  };
  const onFocus = () => {
    clear();
    const focusedAt = now();
    // A timer can fire a hair early; when it does, wait out what is left rather than never cancelling.
    const check = () => {
      timer = undefined;
      if (cancelAfterFocus(focusedAt, now())) onCancel();
      else timer = setTimeout(check, Math.max(1, FOCUS_CANCEL_MS - (now() - focusedAt)));
    };
    timer = setTimeout(check, FOCUS_CANCEL_MS);
  };
  target.addEventListener("focus", onFocus);
  target.addEventListener("blur", clear);
  return () => {
    clear();
    target.removeEventListener("focus", onFocus);
    target.removeEventListener("blur", clear);
  };
}
