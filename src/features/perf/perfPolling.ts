/** A reusable visibility-controlled poller. Hiding stops its timer and invalidates pending results. */
export function createPerfPolling<T>({ poll, intervalMs, onSnapshot }: { poll: () => Promise<T>; intervalMs: number; onSnapshot: (snapshot: T) => void }) {
  let visible = false;
  let generation = 0;
  let inFlight = false;
  let timer: ReturnType<typeof setInterval> | undefined;

  async function sample(): Promise<void> {
    if (!visible || inFlight) return;
    inFlight = true;
    const started = generation;
    try {
      const snapshot = await poll();
      if (visible && started === generation) onSnapshot(snapshot);
    } catch {
      // Keep the last snapshot. A later tick retries.
    } finally {
      inFlight = false;
      // If shown again during an old request, take the fresh sample as soon as it finishes, without overlap.
      if (visible && started !== generation) void sample();
    }
  }

  function setVisible(next: boolean): void {
    if (visible === next) return;
    visible = next;
    generation += 1;
    if (!visible) {
      clearInterval(timer);
      timer = undefined;
      return;
    }
    void sample();
    timer = setInterval(() => void sample(), intervalMs);
  }

  return { setVisible, stop: () => setVisible(false) };
}
