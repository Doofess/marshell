/**
 * Swallows the click that follows a drag's release, and only that one. The browser sends the click right after the release, to
 * the element both the press and the release were on; when a reorder moved the dragged tab from under the pointer there is no
 * such element and no click comes. So an armed guard also disarms itself once the release's event is over (`schedule`, a timer),
 * and the next real click is never the one swallowed.
 */
export function clickGuard(schedule: (run: () => void) => void = (run) => void setTimeout(run, 0)): { arm: () => void; take: () => boolean } {
  let armed = 0;
  return {
    arm() {
      const mine = ++armed;
      schedule(() => {
        if (armed === mine) armed = 0;
      });
    },
    take() {
      if (armed === 0) return false;
      armed = 0;
      return true;
    },
  };
}
