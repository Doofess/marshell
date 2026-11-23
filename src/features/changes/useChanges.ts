import { useCallback, useEffect, useRef, useState } from "react";
import type { ChangesReply } from "../../generated/ChangesReply";
import { readChanges } from "./changesApi";

export const CHANGES_POLL_MS = 2000;

/**
 * What to ask now. Never while a request is out or while the panel isn't shown. Otherwise at once when urgent or when
 * nothing was asked yet, and then every 2,000 ms while the window is focused.
 */
export function changesSchedule(s: { inFlight: boolean; shown: boolean; focused: boolean; urgent: boolean; lastAsk: number | null; now: number }): "none" | "ask" {
  if (s.inFlight || !s.shown) return "none";
  if (s.urgent || s.lastAsk === null) return "ask";
  return s.focused && s.now - s.lastAsk >= CHANGES_POLL_MS ? "ask" : "none";
}

/** The reply to keep: a reply asked for another session than the one shown is dropped, and `same` keeps the last one. */
export function keepReply(tag: { asked: string; shown: string }, current: ChangesReply | null, next: ChangesReply): ChangesReply | null {
  if (tag.asked !== tag.shown) return current;
  return next.kind === "same" ? current : next;
}

const focused = () => typeof document === "undefined" || document.hasFocus();

/**
 * The changes of one session, kept current with the disk: asked once when shown, then every 2,000 ms while the window is
 * focused. Each ask sends the last reply's stamp; `refresh()`, a change of `hideWhitespace` and a change of `session` ask
 * at once with no stamp.
 */
export function useChanges(o: { session: string; shown: boolean; hideWhitespace: boolean }): { reply: ChangesReply | null; noFolder: boolean; busy: boolean; refresh(): void } {
  const { session, shown, hideWhitespace } = o;
  const [state, setState] = useState<{ session: string; reply: ChangesReply | null; noFolder: boolean }>({ session, reply: null, noFolder: false });
  const [busy, setBusy] = useState(false);
  const live = useRef({ session, shown, hideWhitespace });
  live.current = { session, shown, hideWhitespace };
  const run = useRef({ inFlight: false, lastAsk: null as number | null, urgent: true, stamp: null as string | null });
  const stateRef = useRef(state);
  stateRef.current = state;

  // A new session or a new whitespace choice starts over: urgent, no stamp, and no reply of the last session.
  useEffect(() => {
    run.current.urgent = true;
    run.current.stamp = null;
  }, [session, hideWhitespace]);

  useEffect(() => {
    let alive = true;
    const tick = () => {
      const r = run.current;
      const now = Date.now();
      if (changesSchedule({ inFlight: r.inFlight, shown: live.current.shown, focused: focused(), urgent: r.urgent, lastAsk: r.lastAsk, now }) === "none") return;
      const asked = live.current.session;
      const hide = live.current.hideWhitespace;
      const stamp = r.urgent ? null : r.stamp;
      r.inFlight = true;
      r.urgent = false;
      setBusy(true);
      readChanges(asked, { stamp, hide_whitespace: hide })
        .then((reply) => {
          if (!alive || asked !== live.current.session || hide !== live.current.hideWhitespace) return;
          if (reply === "no-folder") {
            r.stamp = null;
            setState({ session: asked, reply: null, noFolder: true });
            return;
          }
          const current = stateRef.current.session === asked ? stateRef.current.reply : null;
          const kept = keepReply({ asked, shown: live.current.session }, current, reply);
          r.stamp = kept?.stamp ?? null;
          setState({ session: asked, reply: kept, noFolder: false });
        })
        .catch(() => {})
        .finally(() => {
          r.inFlight = false;
          r.lastAsk = Date.now();
          if (alive) setBusy(false);
          if (alive) tick();
        });
    };
    const onFocus = () => {
      run.current.urgent = true;
    };
    window.addEventListener("focus", onFocus);
    const timer = window.setInterval(tick, 250);
    tick();
    return () => {
      alive = false;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      run.current.inFlight = false;
    };
  }, []);

  const refresh = useCallback(() => {
    run.current.urgent = true;
    run.current.stamp = null;
  }, []);

  // Never show the last session's changes under the new one.
  const mine = state.session === session;
  return { reply: mine ? state.reply : null, noFolder: mine ? state.noFolder : false, busy, refresh };
}
