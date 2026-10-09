const MAX_SAMPLES = 200;

/** Keypress → output parsed → next frame, in ms. Measured for the phase 1 typing gate (< 30 ms). */
export class LatencyMeter {
  private pendingSince: number | null = null;
  private samples: number[] = [];

  markInput(): void {
    this.pendingSince ??= performance.now();
  }

  /** Call when output was parsed; the sample closes on the next animation frame (the paint). */
  markOutput(): void {
    const since = this.pendingSince;
    if (since === null) return;
    this.pendingSince = null;
    requestAnimationFrame(() => this.record(performance.now() - since));
  }

  record(ms: number): void {
    this.samples.push(ms);
    if (this.samples.length > MAX_SAMPLES) this.samples.shift();
  }

  stats(): { p50: number; p95: number; n: number } {
    const n = this.samples.length;
    if (n === 0) return { p50: 0, p95: 0, n: 0 };
    const sorted = [...this.samples].sort((a, b) => a - b);
    const at = (p: number) => sorted[Math.min(n - 1, Math.ceil((p * n) / 100) - 1)] ?? 0;
    return { p50: at(50), p95: at(95), n };
  }
}
