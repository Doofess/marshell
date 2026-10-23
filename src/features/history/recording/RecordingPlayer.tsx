import "@xterm/xterm/css/xterm.css";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { CastEvent } from "../../../generated/CastEvent";
import type { Platform } from "../../../generated/Platform";
import type { RecordingFrames } from "../../../generated/RecordingFrames";
import type { RecordingPart } from "../../../generated/RecordingPart";
import { ApiError } from "../../../lib/api/client";
import { reasonOf } from "../../../lib/api/sessions";
import { BUNDLED_FONT, fontStack } from "../../terminal/terminalFont";
import { appScheme, resolveTerminalTheme, type TerminalThemeInput } from "../../terminal/terminalTheme";
import { clock, jumpTargets, nextTarget, playbackDelay } from "./castPlayer";
import "./RecordingPlayer.css";

export type FramesQuery = { fromMs?: number; toMs?: number; offset?: number };

const FOLLOW_APP: Omit<TerminalThemeInput, "app"> = { setting: "follow-app", imported: [], protection: true };
const BUNDLED = { family: BUNDLED_FONT, size: 13 };
const SPEEDS = [0.5, 1, 2, 4];
/** `null` keeps every pause. */
const IDLE_CAPS: { value: string; cap: number | null; label: string }[] = [
  { value: "keep", cap: null, label: "Keep" },
  { value: "1000", cap: 1_000, label: "Up to 1 s" },
  { value: "2000", cap: 2_000, label: "Up to 2 s" },
  { value: "5000", cap: 5_000, label: "Up to 5 s" },
];
/** Fetch the next chunk when this many bytes of events or fewer are left. */
const PREFETCH_BYTES = 256 * 1024;
/** The smallest the font gets when fitting a wide recording; beyond it the right edge clips. */
const MIN_FONT = 8;
/** A frame that comes late (a hidden window) plays at most this much catching up. */
const MAX_STEP_MS = 250;

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/** The stretch the slider covers: the conversation's span of the part, or the whole file. */
const rangeOf = (part: RecordingPart, whole: boolean) =>
  whole || part.span_to_ms <= part.span_from_ms ? { from: part.started_ms, to: part.ended_ms } : { from: part.span_from_ms, to: part.span_to_ms };

type Control = { seek: (ms: number, play: boolean) => void; play: (on: boolean) => void };

/**
 * A read-only xterm.js that replays one part of a recording. The core sends a screen for a seek and events after it; the player
 * keeps its own current size (from that screen and the `r` events), since a continued chunk carries only the file header's size.
 * xterm is imported inside the effect, as `ScriptedTerminal` does, so server rendering and the node tests see only the controls.
 */
export function RecordingPlayer({
  part,
  frames,
  turnTimes,
  startAtMs,
  whole,
  theme = FOLLOW_APP,
  font = BUNDLED,
  platform = "windows",
}: {
  part: RecordingPart;
  /** The part's frames: a seek (`fromMs`) or a continuation (`offset`). */
  frames: (q: FramesQuery) => Promise<RecordingFrames>;
  /** The turns' start times, as extra jump targets. */
  turnTimes: number[];
  /** Where to start, playing; null starts paused at the beginning of the range. */
  startAtMs: number | null;
  /** Play the whole file, not only the stretch the conversation covers. */
  whole: boolean;
  theme?: Omit<TerminalThemeInput, "app">;
  font?: { family: string; size: number };
  platform?: Platform;
}) {
  const range = rangeOf(part, whole);
  const total = range.to - range.from;
  const host = useRef<HTMLDivElement>(null);
  const ctl = useRef<Control | null>(null);
  const termRef = useRef<import("@xterm/xterm").Terminal | null>(null);
  const [ready, setReady] = useState(false);
  const [pos, setPos] = useState(() => clamp(startAtMs ?? range.from, range.from, range.to));
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [idle, setIdle] = useState("2000");
  const [problem, setProblem] = useState<string | null>(null);
  const cap = IDLE_CAPS.find((c) => c.value === idle)?.cap ?? null;

  // What the engine reads while it runs; assigned in an effect so a render never changes it mid-frame.
  const live = useRef({ part, frames, range, speed, cap, font, theme, pos, playing });
  useEffect(() => {
    live.current = { part, frames, range, speed, cap, font, theme, pos, playing };
  });

  const targets = useMemo(
    () => jumpTargets(part, turnTimes).filter((t) => t >= range.from && t <= range.to),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [part, turnTimes, range.from, range.to],
  );

  // The terminal and the playback engine. Made once per player; a new part reseeks it.
  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let cancelled = false;
    let dispose = () => {};

    void (async () => {
      const [{ Terminal }, { loadRenderer }] = await Promise.all([import("@xterm/xterm"), import("../../terminal/renderer")]);
      if (cancelled) return;
      const resolve = () => resolveTerminalTheme({ ...live.current.theme, app: appScheme(el) });
      const first = resolve();
      let cols = live.current.part.cols;
      let rows = live.current.part.rows;
      const term = new Terminal({
        fontFamily: fontStack(live.current.font.family),
        fontSize: live.current.font.size,
        cols,
        rows,
        disableStdin: true,
        cursorBlink: false,
        cursorInactiveStyle: "block",
        scrollback: 0,
        theme: first.theme,
        minimumContrastRatio: first.minimumContrastRatio,
      });
      term.open(el);
      loadRenderer(term, platform, () => {});
      termRef.current = term;
      el.style.backgroundColor = first.theme.background ?? "";

      /** The width of one cell as a fraction of the font size, measured from the font in use. */
      const cellRatio = () => {
        const ctx = document.createElement("canvas").getContext("2d");
        if (!ctx) return 0.6;
        ctx.font = `100px ${fontStack(live.current.font.family)}`;
        const w = ctx.measureText("M").width / 100;
        return w > 0 ? w : 0.6;
      };
      /** Fits the recording's columns into the host width: the terminal font size down to 8 px, then it clips. */
      const fit = () => {
        const style = getComputedStyle(el);
        const width = el.clientWidth - parseFloat(style.paddingInlineStart || "0") - parseFloat(style.paddingInlineEnd || "0");
        if (width <= 0) return;
        const size = clamp(Math.floor(width / (cols * cellRatio())), MIN_FONT, Math.max(MIN_FONT, live.current.font.size));
        if (term.options.fontSize !== size) term.options.fontSize = size;
      };
      const resizeTo = (c: number, r: number) => {
        if (!(c >= 1 && r >= 1 && c <= 1000 && r <= 1000)) return;
        cols = c;
        rows = r;
        term.resize(c, r);
        fit();
      };
      const applyTheme = () => {
        const o = resolve();
        term.options.theme = o.theme;
        term.options.minimumContrastRatio = o.minimumContrastRatio;
        el.style.backgroundColor = o.theme.background ?? "";
      };
      const prefers = matchMedia("(prefers-color-scheme: dark)");
      prefers.addEventListener("change", applyTheme);
      const watch = new MutationObserver(applyTheme);
      watch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
      const observer = new ResizeObserver(fit);
      observer.observe(el);
      fit();

      // Playback state.
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

      const fail = (e: unknown) => setProblem(e instanceof ApiError ? reasonOf(e) : e instanceof Error ? e.message : String(e));
      const stop = () => {
        on = false;
        setPlaying(false);
        cancelAnimationFrame(raf);
        raf = 0;
      };
      const pump = () => {
        if (nextOffset === null || inflight || remaining > PREFETCH_BYTES) return;
        inflight = true;
        const mine = token;
        const to = live.current.range.to + 1;
        live.current
          .frames({ offset: nextOffset, toMs: to })
          .then((res) => {
            if (mine !== token || cancelled) return;
            queue.push(...res.events);
            remaining += res.events.reduce((n, e) => n + e.data.length, 0);
            nextOffset = res.next_offset;
          })
          .catch((e) => {
            if (mine === token) {
              nextOffset = null;
              fail(e);
            }
          })
          .finally(() => {
            if (mine === token) inflight = false;
          });
      };
      const apply = (ev: CastEvent) => {
        if (ev.code === "o") term.write(ev.data);
        else if (ev.code === "r") {
          const m = /^(\d+)x(\d+)$/.exec(ev.data);
          if (m) resizeTo(Number(m[1]), Number(m[2]));
        }
      };
      const loop = (ts: number) => {
        raf = 0;
        if (!on || cancelled) return;
        const { range: r, speed: sp, cap: idleCap } = live.current;
        budget += lastTs === null ? 0 : Math.min(ts - lastTs, MAX_STEP_MS);
        lastTs = ts;
        for (;;) {
          const ev = queue[qi];
          if (!ev) {
            if (nextOffset === null && !inflight) {
              setPos(r.to);
              stop();
              return;
            }
            break;
          }
          if (ev.at_ms > r.to) {
            setPos(r.to);
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
        setPos(lastAt);
        raf = requestAnimationFrame(loop);
      };
      const play = (go: boolean) => {
        if (go === on) return;
        on = go;
        setPlaying(go);
        if (go) {
          lastTs = null;
          raf = requestAnimationFrame(loop);
        } else {
          cancelAnimationFrame(raf);
          raf = 0;
        }
      };
      const seek = (ms: number, go: boolean) => {
        const r = live.current.range;
        const target = clamp(ms, r.from, r.to);
        const mine = ++token;
        stop();
        setPos(target);
        setProblem(null);
        inflight = false;
        live.current.frames({ fromMs: target, toMs: r.to + 1 }).then(
          (res) => {
            if (mine !== token || cancelled) return;
            term.reset();
            if (res.screen !== null) {
              resizeTo(res.cols, res.rows);
              term.write(res.screen);
            }
            queue = res.events;
            qi = 0;
            remaining = res.events.reduce((n, e) => n + e.data.length, 0);
            nextOffset = res.next_offset;
            lastAt = target;
            budget = 0;
            pump();
            if (go) play(true);
          },
          (e) => {
            if (mine === token) fail(e);
          },
        );
      };
      ctl.current = { seek, play };
      setReady(true);

      dispose = () => {
        token++;
        cancelAnimationFrame(raf);
        prefers.removeEventListener("change", applyTheme);
        watch.disconnect();
        observer.disconnect();
        termRef.current = null;
        ctl.current = null;
        term.dispose();
      };
    })();

    return () => {
      cancelled = true;
      setReady(false);
      dispose();
    };
  }, [platform]);

  // The font follows the settings.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    term.options.fontFamily = fontStack(font.family);
    term.options.fontSize = font.size;
  }, [font.family, font.size, ready]);

  // A new part, a new "play from here", or the switch: seek. Only the switch keeps the current position.
  const previous = useRef<{ key: string; whole: boolean } | null>(null);
  useEffect(() => {
    if (!ready || !ctl.current) return;
    const key = `${part.file}|${part.started_ms}|${startAtMs}`;
    const last = previous.current;
    previous.current = { key, whole };
    const r = live.current.range;
    if (last && last.key === key && last.whole !== whole) ctl.current.seek(live.current.pos, live.current.playing);
    else ctl.current.seek(startAtMs ?? r.from, startAtMs !== null);
  }, [ready, part.file, part.started_ms, startAtMs, whole]); // eslint-disable-line react-hooks/exhaustive-deps

  const toggle = () => {
    if (!ctl.current) return;
    if (playing) ctl.current.play(false);
    else if (pos >= range.to) ctl.current.seek(range.from, true);
    else ctl.current.play(true);
  };
  const jump = (by: -1 | 1) => {
    const t = nextTarget(targets, pos, by);
    if (t !== null) ctl.current?.seek(t, playing);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== " ") return;
    e.preventDefault();
    toggle();
  };

  const rel = clamp(pos - range.from, 0, total);
  return (
    <div className="player" role="group" aria-label="Recording player">
      <div ref={host} className="player__screen" role="group" aria-label="Recording screen" onKeyDown={onKeyDown} />
      {problem !== null && <p className="player__problem">{problem}</p>}
      <div className="player__bar">
        <button type="button" className="player__button" onClick={toggle}>
          {playing ? "Pause" : "Play"}
        </button>
        <button type="button" className="player__button" aria-disabled={nextTarget(targets, pos, -1) === null} onClick={() => jump(-1)}>
          Previous mark
        </button>
        <button type="button" className="player__button" aria-disabled={nextTarget(targets, pos, 1) === null} onClick={() => jump(1)}>
          Next mark
        </button>
        <input
          type="range"
          className="player__slider"
          aria-label="Position"
          aria-valuetext={`${clock(rel, total)} of ${clock(total, total)}`}
          min={0}
          max={total}
          step={1000}
          value={rel}
          onChange={(e) => ctl.current?.seek(range.from + Number(e.target.value), playing)}
        />
        <span className="player__time" aria-hidden="true">
          {clock(rel, total)} / {clock(total, total)}
        </span>
        <label className="player__field">
          <span>Speed</span>
          <select className="player__select" aria-label="Speed" value={speed} onChange={(e) => setSpeed(Number(e.target.value))}>
            {SPEEDS.map((s) => (
              <option key={s} value={s}>
                {`${s}×`}
              </option>
            ))}
          </select>
        </label>
        <label className="player__field">
          <span>Idle time</span>
          <select className="player__select" aria-label="Idle time" value={idle} onChange={(e) => setIdle(e.target.value)}>
            {IDLE_CAPS.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </label>
      </div>
    </div>
  );
}
