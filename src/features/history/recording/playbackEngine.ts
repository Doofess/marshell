import type { CastEvent } from "../../../generated/CastEvent";
import type { RecordingFrames } from "../../../generated/RecordingFrames";
import { playbackDelay } from "./castPlayer";

export type FramesQuery = { fromMs?: number; toMs?: number; offset?: number };

/** Fetch the next chunk when this many bytes of events or fewer are left. */
const PREFETCH_BYTES = 256 * 1024;
/** A frame that comes late (a hidden window) plays at most this much catching up. */
const MAX_STEP_MS = 250;

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/** Everything the engine takes from the browser and the terminal, so that it runs in node too. */
export type PlaybackDeps = {
  /** The part's frames: a seek (`fromMs`) or a continuation (`offset`). */
  frames: (q: FramesQuery) => Promise<RecordingFrames>;
  /** Read at each use, since the range, speed and idle cap change while it runs. */
  range: () => { from: number; to: number };
  speed: () => number;
  idleCap: () => number | null;
  write: (data: string) => void;
  reset: () => void;
  resizeTo: (cols: number, rows: number) => void;
  onPos: (ms: number) => void;
  onPlaying: (on: boolean) => void;
  onProblem: (e: unknown) => void;
  requestFrame: (cb: (ts: number) => void) => number;
  cancelFrame: (id: number) => void;
};

export type PlaybackEngine = {
  seek: (ms: number, play: boolean) => void;
  play: (on: boolean) => void;
  dispose: () => void;
};

/**
 * The playback state machine of the recording player: a queue of events fed by a seek and its continuations, played out on
 * frames by the recorded gaps. The terminal and the clock are injected.
 */
export function createPlaybackEngine(deps: PlaybackDeps): PlaybackEngine {
  let queue: CastEvent[] = [];
  let qi = 0;
  let remaining = 0;
  let nextOffset: number | null = null;
  let inflight = false;
  let token = 0;
  let lastAt = 0;
  let budget = 0;
  let lastTs: number | null = null;
  let raf = 0;
  let on = false;
  let disposed = false;
  /** A seek is waiting for its answer: the queue is empty and nothing plays until the answer fills it. */
  let pending = false;
  /** Whether playback is to go on when the pending seek's answer arrives. */
  let resume = false;

  const stop = () => {
    on = false;
    deps.onPlaying(false);
    deps.cancelFrame(raf);
    raf = 0;
  };
  const pump = () => {
    if (pending || nextOffset === null || inflight || remaining > PREFETCH_BYTES) return;
    inflight = true;
    const mine = token;
    const to = deps.range().to + 1;
    deps
      .frames({ offset: nextOffset, toMs: to })
      .then((res) => {
        if (mine !== token || disposed) return;
        queue.push(...res.events);
        remaining += res.events.reduce((n, e) => n + e.data.length, 0);
        nextOffset = res.next_offset;
      })
      .catch((e) => {
        if (mine === token) {
          nextOffset = null;
          deps.onProblem(e);
        }
      })
      .finally(() => {
        if (mine === token) inflight = false;
      });
  };
  const apply = (ev: CastEvent) => {
    if (ev.code === "o") deps.write(ev.data);
    else if (ev.code === "r") {
      const m = /^(\d+)x(\d+)$/.exec(ev.data);
      if (m) deps.resizeTo(Number(m[1]), Number(m[2]));
    }
  };
  const loop = (ts: number) => {
    raf = 0;
    if (!on || disposed) return;
    const r = deps.range();
    const sp = deps.speed();
    const idleCap = deps.idleCap();
    if (pending) {
      // Keep asking for frames, but let no time pile up for the answer to spend at once.
      lastTs = null;
      raf = deps.requestFrame(loop);
      return;
    }
    budget += lastTs === null ? 0 : Math.min(ts - lastTs, MAX_STEP_MS);
    lastTs = ts;
    for (;;) {
      const ev = queue[qi];
      if (!ev) {
        if (nextOffset === null && !inflight) {
          deps.onPos(r.to);
          stop();
          return;
        }
        break;
      }
      if (ev.at_ms > r.to) {
        deps.onPos(r.to);
        stop();
        return;
      }
      const delay = playbackDelay(ev.at_ms - lastAt, sp, idleCap);
      if (budget < delay) break;
      budget -= delay;
      apply(ev);
      lastAt = ev.at_ms;
      qi++;
      remaining -= ev.data.length;
    }
    if (qi > 2000) {
      queue = queue.slice(qi);
      qi = 0;
    }
    pump();
    deps.onPos(lastAt);
    raf = deps.requestFrame(loop);
  };
  const play = (go: boolean) => {
    if (pending) resume = go;
    if (go === on) return;
    on = go;
    deps.onPlaying(go);
    if (go) {
      lastTs = null;
      raf = deps.requestFrame(loop);
    } else {
      deps.cancelFrame(raf);
      raf = 0;
    }
  };
  const seek = (ms: number, go: boolean) => {
    const r = deps.range();
    const target = clamp(ms, r.from, r.to);
    const mine = ++token;
    stop();
    deps.onPos(target);
    // The old queue and offset belong to the position before the seek: none of it may play or be continued.
    queue = [];
    qi = 0;
    remaining = 0;
    nextOffset = null;
    inflight = false;
    pending = true;
    resume = go;
    deps.frames({ fromMs: target, toMs: r.to + 1 }).then(
      (res) => {
        if (mine !== token || disposed) return;
        pending = false;
        deps.reset();
        if (res.screen !== null) {
          deps.resizeTo(res.cols, res.rows);
          deps.write(res.screen);
        }
        queue = res.events;
        qi = 0;
        remaining = res.events.reduce((n, e) => n + e.data.length, 0);
        nextOffset = res.next_offset;
        lastAt = target;
        budget = 0;
        pump();
        if (resume) play(true);
      },
      (e) => {
        if (mine !== token) return;
        pending = false;
        stop();
        deps.onProblem(e);
      },
    );
  };
  const dispose = () => {
    disposed = true;
    token++;
    deps.cancelFrame(raf);
  };
  return { seek, play, dispose };
}
