import { describe, expect, it } from "vitest";
import type { CastEvent } from "../../../generated/CastEvent";
import type { RecordingFrames } from "../../../generated/RecordingFrames";
import { createPlaybackEngine, type FramesQuery } from "./playbackEngine";

const out = (at_ms: number, data: string): CastEvent => ({ at_ms, code: "o", data });
const answer = (events: CastEvent[], next_offset: number | null = null): RecordingFrames => ({ cols: 80, rows: 24, screen: null, events, next_offset });
const flush = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};

/** An engine with a manual frame scheduler, and frames() answers that the test resolves by hand. */
function harness(opts: { to?: number; cap?: number | null } = {}) {
  const range = { from: 1_000, to: opts.to ?? 100_000 };
  const calls: { q: FramesQuery; resolve: (r: RecordingFrames) => void }[] = [];
  const written: string[] = [];
  const positions: number[] = [];
  const playing: boolean[] = [];
  const problems: unknown[] = [];
  let frameCbs = new Map<number, (ts: number) => void>();
  let nextId = 1;
  let now = 0;
  const engine = createPlaybackEngine({
    frames: (q) => new Promise<RecordingFrames>((resolve) => calls.push({ q, resolve })),
    range: () => range,
    speed: () => 1,
    idleCap: () => (opts.cap === undefined ? null : opts.cap),
    write: (d) => written.push(d),
    reset: () => {},
    resizeTo: () => {},
    onPos: (ms) => positions.push(ms),
    onPlaying: (on) => playing.push(on),
    onProblem: (e) => problems.push(e),
    requestFrame: (cb) => {
      frameCbs.set(nextId, cb);
      return nextId++;
    },
    cancelFrame: (id) => {
      frameCbs.delete(id);
    },
  });
  /** Runs `n` frames, `step` ms apart, letting promises settle after each. */
  const advance = async (n: number, step = 250) => {
    for (let i = 0; i < n; i++) {
      now += step;
      const cbs = [...frameCbs.values()];
      frameCbs = new Map();
      for (const cb of cbs) cb(now);
      await flush();
    }
  };
  const answerCall = async (i: number, r: RecordingFrames) => {
    calls[i]!.resolve(r);
    await flush();
  };
  return { engine, calls, written, positions, playing, problems, advance, answerCall };
}

describe("playback engine", () => {
  it("drops a continuation that was in flight when a seek started", async () => {
    const h = harness();
    h.engine.seek(1_000, true);
    await h.answerCall(0, answer([out(1_100, "a")], 10)); // asks for offset 10 at once
    await h.advance(2);
    h.engine.seek(5_000, true); // Next mark while playing,
    h.engine.play(true); // then Play before the answer
    await h.advance(2);
    const seekCall = h.calls.findIndex((c) => c.q.fromMs === 5_000);
    await h.answerCall(seekCall, answer([out(5_100, "s1")], 20));
    for (let i = 0; i < h.calls.length; i++) {
      if (h.calls[i]!.q.offset === 10) await h.answerCall(i, answer([out(1_500, "old")], 99));
    }
    await h.advance(6);
    expect(h.written).not.toContain("old");
    const offsets = h.calls.map((c) => c.q.offset).filter((o) => o !== undefined);
    expect(offsets[offsets.length - 1]).toBe(20);
    expect(offsets).not.toContain(99);
  });

  it("plays from the seek target when Play is pressed before the seek answer arrives", async () => {
    const h = harness();
    h.engine.seek(1_000, false);
    await h.answerCall(0, answer([out(1_100, "old1"), out(1_200, "old2"), out(1_300, "old3")]));
    h.positions.length = 0;
    h.engine.seek(50_000, false);
    h.engine.play(true);
    await h.advance(8);
    expect(h.written).toEqual([]);
    expect(Math.min(...h.positions)).toBeGreaterThanOrEqual(50_000);
    await h.answerCall(1, answer([out(50_100, "n1"), out(50_200, "n2")]));
    await h.advance(4);
    expect(h.written).toEqual(["n1", "n2"]);
    expect(Math.min(...h.positions)).toBeGreaterThanOrEqual(50_000);
  });

  it("keeps playback off when Pause is pressed before the seek answer arrives", async () => {
    const h = harness();
    h.engine.seek(5_000, true);
    h.engine.play(true);
    h.engine.play(false);
    await h.answerCall(0, answer([out(5_100, "n1")]));
    await h.advance(4);
    expect(h.written).toEqual([]);
    expect(h.playing[h.playing.length - 1]).toBe(false);
  });

  it("stops at the range end", async () => {
    const h = harness({ to: 600_000 });
    h.engine.seek(1_000, true);
    await h.answerCall(0, answer([out(1_100, "a"), out(700_000, "late")]));
    await h.advance(6);
    expect(h.written).toEqual(["a"]);
    expect(h.positions[h.positions.length - 1]).toBe(600_000);
    expect(h.playing[h.playing.length - 1]).toBe(false);
  });

  it("caps idle gaps", async () => {
    const h = harness({ to: 600_000, cap: 1_000 });
    h.engine.seek(1_000, true);
    await h.answerCall(0, answer([out(2_000, "a"), out(500_000, "b")]));
    await h.advance(3);
    expect(h.written).toEqual([]);
    await h.advance(2);
    expect(h.written).toEqual(["a"]);
    await h.advance(2);
    expect(h.written).toEqual(["a"]); // the 498 s gap is held to 1 s, not skipped
    await h.advance(4);
    expect(h.written).toEqual(["a", "b"]);
  });
});
