import type { Report, Size } from "./browserApi";

type Queued = { measure: () => Report; direct: boolean };

const same = (a: Size, b: Size) => a.width === b.width && a.height === b.height;

/**
 * Sends the page's box to the shell without piling up. One report is in flight; at most one waits behind it, and it measures
 * when its turn comes, so what it sends is current (S12 finding 20). A report at the window's last size is sent after the app
 * has painted that frame, so the page doesn't move ahead of the app's layout (finding 21); one at a new size goes at once and
 * drops a report still waiting for a paint. While the app's own layout moves, a report comes every frame at the same size; each
 * one joins the paint wait already running instead of starting its own, which would never end. `cancel` drops whatever hasn't
 * gone yet, for a slot that is gone.
 */
export function createPlacer(send: (r: Report) => Promise<void>, nextPaint: () => Promise<void>): { report(measure: () => Report): void; cancel(): void } {
  let sending = false;
  let queued: Queued | null = null;
  // The paint wait that is running, with the measure it will send: the newest report's.
  let pending: { measure: () => Report } | null = null;
  let last: Size | null = null;

  function transmit(r: Report): void {
    sending = true;
    last = r.win;
    send(r)
      .catch(() => {})
      .then(() => {
        sending = false;
        const next = queued;
        queued = null;
        if (!next) return;
        if (next.direct) transmit(next.measure());
        else process(next.measure);
      });
  }

  function process(measure: () => Report): void {
    const r = measure();
    if (last === null || !same(r.win, last)) {
      pending = null;
      transmit(r);
      return;
    }
    if (pending) {
      pending.measure = measure;
      return;
    }
    const wait = { measure };
    pending = wait;
    nextPaint()
      .catch(() => {})
      .then(() => {
        if (pending !== wait) return;
        pending = null;
        if (sending) queued = { measure: wait.measure, direct: true };
        else transmit(wait.measure());
      });
  }

  return {
    report(measure) {
      if (sending) {
        queued = { measure, direct: false };
        return;
      }
      process(measure);
    },
    cancel() {
      pending = null;
      queued = null;
    },
  };
}
