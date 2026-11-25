/** What an empty state shows: 0 everything; 1 no mark and no help line; 2 no list either. The heading and the buttons always stay. */
export type FitLevel = 0 | 1 | 2;

/** The lowest level whose height is at most `room`, asking `heightAt` for no level after the one that fits; 2 when none fits. */
export function firstFit(room: number, heightAt: (level: FitLevel) => number): FitLevel {
  if (heightAt(0) <= room) return 0;
  if (heightAt(1) <= room) return 1;
  return 2;
}

/**
 * Sets `panel`'s `data-level` to the first level whose column fits the panel's height. It writes the attribute itself, in three
 * synchronous tries before paint, so there is no React state to loop on and React never resets it (it is not in the JSX).
 */
export function fitEmpty(panel: HTMLElement, column: HTMLElement): void {
  panel.dataset.level = String(
    firstFit(panel.clientHeight, (l) => {
      panel.dataset.level = String(l);
      return column.offsetHeight;
    }),
  );
}
