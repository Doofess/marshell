import { useEffect, useRef, useState } from "react";

/** Every layout switch: 200 ms on this curve. The same numbers as `--dur-drawer` and `--ease-out` (a test reads the tokens). */
export const LAYOUT_MOTION = { ms: 200, curve: [0.2, 0.8, 0.2, 1] } as const;

type Curve = readonly [number, number, number, number];

/** One coordinate of a cubic bezier that starts at 0 and ends at 1, with control values `a` and `b`, at parameter `u`. */
const bezier = (a: number, b: number, u: number) => 3 * a * (1 - u) * (1 - u) * u + 3 * b * (1 - u) * u * u + u * u * u;
const slope = (a: number, b: number, u: number) => 3 * a * (1 - u) * (1 - u) + 6 * (b - a) * (1 - u) * u + 3 * (1 - b) * u * u;

/** The curve's y at time fraction t (0 to 1), solved as CSS's cubic-bezier is: find u where x(u) = t, then take y(u). */
export function easeAt(t: number, curve: Curve = LAYOUT_MOTION.curve): number {
  if (!(t > 0)) return 0;
  if (t >= 1) return 1;
  const [x1, y1, x2, y2] = curve;
  let u = t;
  // Newton's method converges in a few steps on a curve like this one.
  for (let i = 0; i < 8; i++) {
    const err = bezier(x1, x2, u) - t;
    if (Math.abs(err) < 1e-7) return bezier(y1, y2, u);
    const d = slope(x1, x2, u);
    if (Math.abs(d) < 1e-6) break;
    u -= err / d;
  }
  // Where the slope is flat or Newton left the curve, bisect: x(u) rises with u.
  let lo = 0;
  let hi = 1;
  u = t;
  for (let i = 0; i < 60; i++) {
    const x = bezier(x1, x2, u);
    if (Math.abs(x - t) < 1e-7) break;
    if (x < t) lo = u;
    else hi = u;
    u = (lo + hi) / 2;
  }
  return bezier(y1, y2, u);
}

export type Motion = { from: number; startedAt: number } | null;

/** The value to draw at `now`: `target` when idle or done, else between `from` and the live `target`. */
export function motionValue(m: Motion, target: number, now: number): { value: number; done: boolean } {
  if (m === null) return { value: target, done: true };
  const p = (now - m.startedAt) / LAYOUT_MOTION.ms;
  if (p >= 1) return { value: target, done: true };
  const eased = p > 0 ? easeAt(p) : 0;
  return { value: m.from + (target - m.from) * eased, done: false };
}

/**
 * What a render does with its motion: a changed `key` starts one from the value last drawn, so a switch that interrupts another
 * turns round from where it is. The same key keeps the motion as it is, and a changed target is followed at once. Reduced
 * motion starts none.
 */
export function nextMotion(prev: { key: string; motion: Motion; drawn: number }, key: string, now: number, reduced: boolean): Motion {
  if (key === prev.key) return prev.motion;
  return reduced ? null : { from: prev.drawn, startedAt: now };
}

/** The system's setting or the app's own switch, read when a motion would start (the same two LiveApp reads). */
function reducedMotion(): boolean {
  if (typeof window === "undefined") return true;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.motion === "reduce";
}

const clock = () => (typeof performance !== "undefined" ? performance.now() : Date.now());

/**
 * The animated number for a layout value. `key` names the discrete state; only a change of key moves, so a drag, which changes
 * only `target`, is followed at once. The first render never moves. While a motion runs the component renders on every frame.
 */
export function useLayoutMotion(key: string, target: number): { value: number; moving: boolean } {
  const state = useRef<{ key: string; motion: Motion; drawn: number }>({ key, motion: null, drawn: target });
  const [, frame] = useState(0);
  const now = clock();
  const s = state.current;
  if (s.key !== key) {
    s.motion = nextMotion(s, key, now, reducedMotion());
    s.key = key;
  }
  const { value, done } = motionValue(s.motion, target, now);
  if (done) s.motion = null;
  s.drawn = value;
  const moving = !done;
  useEffect(() => {
    if (!moving) return;
    const id = requestAnimationFrame(() => frame((n) => n + 1));
    return () => cancelAnimationFrame(id);
  });
  return { value, moving };
}
