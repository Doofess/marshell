import type { Os } from "../../lib/keymap";
/** Where the launcher's highlighted option is being moved: the folder field (a text input), or the shell or agent list. */
export type OpenList = "shells" | "agents" | "folder";

/**
 * The keys that move the highlighted option. In the folder field Left and Right belong to the caret, so a typo in the
 * query can be fixed; only the shell and agent lists, which have no caret, use all four arrows.
 */
export function listNavKeys(open: OpenList | null): readonly string[] {
  if (open === "shells" || open === "agents") return ["ArrowDown", "ArrowUp", "ArrowRight", "ArrowLeft"];
  if (open === "folder") return ["ArrowDown", "ArrowUp"];
  return [];
}

/** The next highlighted index for an arrow key, wrapping, and recovering from a stale index. */
export function stepActive(index: number, key: string, size: number): number {
  if (size <= 0) return 0;
  const step = key === "ArrowDown" || key === "ArrowRight" ? 1 : -1;
  const from = Number.isFinite(index) ? Math.min(Math.max(0, Math.floor(index)), size - 1) : 0;
  return (from + step + size) % size;
}

/** Which of the launcher's three lists is open: the folder field when `query` is not null, the shell list, the agent list. At most one is. */
export type OpenLists = { query: string | null; shellsOpen: boolean; agentsOpen: boolean };

/** Opens or closes the shell or agent list. The launcher shows one list at a time, so opening one closes the others. */
export function openList(lists: OpenLists, which: "shells" | "agents", open: boolean): OpenLists {
  return {
    query: open ? null : lists.query,
    shellsOpen: which === "shells" ? open : open ? false : lists.shellsOpen,
    agentsOpen: which === "agents" ? open : open ? false : lists.agentsOpen,
  };
}

/** Sets the folder field's text, null to close it. Opening it closes the other lists, which would otherwise come back after a pick. */
export function openFolder(lists: OpenLists, query: string | null): OpenLists {
  return query === null ? { ...lists, query } : { query, shellsOpen: false, agentsOpen: false };
}

/**
 * Closing the folder field swaps an input for a button, so the browser drops focus on the page and the sheet's keys (Esc, Enter)
 * stop arriving. True when the folder chip should take focus back: the field just closed and nothing real has focus.
 */
export function focusFellOut({ wasOpen, isOpen, activeIsBody }: { wasOpen: boolean; isOpen: boolean; activeIsBody: boolean }): boolean {
  return wasOpen && !isOpen && activeIsBody;
}

/** The open list, if any: the one Esc closes, and the one Alt+N closes before it moves to Name (its Enter and arrows would act on a list left behind). */
export function listToClose({ folderOpen, shellsOpen, agentsOpen }: { folderOpen: boolean; shellsOpen: boolean; agentsOpen: boolean }): OpenList | null {
  return folderOpen ? "folder" : shellsOpen ? "shells" : agentsOpen ? "agents" : null;
}

/** What Esc does, and a press on the dimmed window around the sheet: the confirm view goes back, an open list closes, else the launcher closes. */
export function escapeStep({ confirm, open }: { confirm: boolean; open: OpenList | null }): "back" | OpenList | "close" {
  return confirm ? "back" : (open ?? "close");
}

/** A press on the dimmed window around the sheet: Esc, for the main button only. preventDefault keeps the focus off the page body meanwhile. */
export function backdropPress(e: { button: number; preventDefault: () => void }, escape: () => void) {
  if (e.button !== 0) return;
  e.preventDefault();
  escape();
}

/**
 * The sheet holds the focus while nothing in it can (the lists still loading, Start disabled): once the prompt is there, or Start is
 * enabled, the focus moves on to it, so typing and Enter work. A sheet that doesn't hold the focus moves nothing.
 */
export function sheetHandoff({ onSheet, prompt, canStart }: { onSheet: boolean; prompt: boolean; canStart: boolean }): "prompt" | "start" | null {
  if (!onSheet) return null;
  return prompt ? "prompt" : canStart ? "start" : null;
}

/**
 * The sheet can hold the focus, so a press on a blank spot in it moves the focus there from the control it was on, and typing would
 * go nowhere. The control the focus left, inside the sheet, gets it back; focus that came from outside (the opening) stays.
 */
export function sheetRefocus<T>(target: unknown, sheet: { contains(n: unknown): boolean }, related: T | null): T | null {
  return target === sheet && related !== null && related !== sheet && sheet.contains(related) ? related : null;
}

/** The letter a focused chip picks by. A held Alt, Ctrl or Meta makes the key a shortcut (Alt+N, Alt+A), never a pick; Shift is a capital. */
export function chipLetter(e: { key: string; altKey: boolean; ctrlKey: boolean; metaKey: boolean }): string | null {
  return e.altKey || e.ctrlKey || e.metaKey ? null : e.key;
}

/** Alt+A, by its physical key so it sits in the same place on every layout: toggles the Administrator chip from anywhere in the sheet. */
export function adminKey(e: { code: string; altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): boolean {
  return e.code === "KeyA" && e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
}

/** Alt+A is the chip's key only while the chip works; when it is off (macOS, Linux, a CLI that can't run elevated) the key is left alone, as Option+A is a letter on a Mac. */
export function adminKeyClaimed(e: { code: string; altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }, enabled: boolean): boolean {
  return enabled && adminKey(e);
}

/** Alt+N, by its physical key: moves to the Name field from anywhere in the sheet. */
export function nameKey(e: { code: string; altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): boolean {
  return e.code === "KeyN" && e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
}

/** Alt+N is the Name field's key except on a Mac, where Option+N is the dead key for a tilde (as Option+A is left alone for the Administrator chip). Tab still reaches the field. */
export function nameKeyClaimed(e: Parameters<typeof nameKey>[0], os: Os): boolean {
  return os !== "mac" && nameKey(e);
}
