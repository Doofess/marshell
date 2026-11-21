import type { ViewportId } from "./viewport";

/**
 * Where the focus goes once the viewport chip's menu has closed and its pick is on screen: `picked` is the chosen size (null is
 * Responsive, which shows no chip, undefined is no pick). The ⋯ button of the Browser panel takes the focus when no chip is left
 * for the front tab. `lookup` searches the front tab's own panel only: tabs behind it stay mounted but hidden, and a hidden element
 * takes no focus.
 */
export function viewportReturn<T extends { isConnected: boolean }>(
  picked: ViewportId | null | undefined,
  opener: T | null,
  lookup: { chip(): T | null; menu(): T | null },
): T | null {
  if (picked === null) return lookup.menu();
  if (opener?.isConnected) return opener;
  return lookup.chip() ?? lookup.menu();
}
