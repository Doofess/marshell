import { useCallback, useEffect, useRef, useState } from "react";
import type { GithubReply } from "../../generated/GithubReply";
import { readGithub } from "./githubApi";

export const GITHUB_DEFAULT_MS = 30_000;

/**
 * What to ask now. Never while a request is out, the panel is hidden, the window is unfocused or the setting is off.
 * Otherwise "ask" (GitHub, fresh) when nothing was asked yet or `nextMs` has passed since the last ask (`refresh()` makes
 * the last ask infinitely old), and "local" (recount without asking GitHub) when only the head moved.
 */
export function githubSchedule(s: { inFlight: boolean; shown: boolean; focused: boolean; on: boolean; lastAsk: number; nextMs: number; now: number; headMoved: boolean; asked: boolean }): "none" | "ask" | "local" {
  if (s.inFlight || !s.shown || !s.focused || !s.on) return "none";
  if (!s.asked || s.now - s.lastAsk >= s.nextMs) return "ask";
  return s.headMoved ? "local" : "none";
}

/** What a local recount watches: the branch and the commit. A new branch at the same commit can have another pull request. */
export function recountKey(listed: { branch: string | null; head: string | null } | null): string | null {
  return listed === null ? null : `${listed.branch ?? ""} ${listed.head ?? ""}`;
}

/** When GitHub was last asked. A local recount leaves it alone, so GitHub is still asked on time while commits keep coming. */
export function askedAt(mode: "ask" | "local", lastAsk: number, now: number): number {
  return mode === "ask" ? now : lastAsk;
}

const focused = () => typeof document === "undefined" || document.hasFocus();

type State = { tab: string | null; reply: GithubReply | null };

/**
 * What GitHub says about one session's checkout. Holds `reply: null` and asks nothing while `on` is false or `tab` is
 * null; a new tab clears the reply at once, and an answer for an earlier tab is dropped. `refresh()` asks GitHub now.
 */
/** `head` is `recountKey` of what the panel lists: a change in it recounts at once. */
export function useGithub(tab: string | null, shown: boolean, head: string | null, on: boolean): { reply: GithubReply | null; refresh: () => void } {
  const [state, setState] = useState<State>({ tab, reply: null });
  const live = useRef({ tab, shown, head, on });
  live.current = { tab, shown, head, on };
  const run = useRef({ inFlight: false, asked: false, lastAsk: 0, nextMs: GITHUB_DEFAULT_MS, answeredHead: null as string | null, urgent: false });

  useEffect(() => {
    run.current = { inFlight: false, asked: false, lastAsk: 0, nextMs: GITHUB_DEFAULT_MS, answeredHead: null, urgent: false };
    setState({ tab, reply: null });
  }, [tab, on]);

  useEffect(() => {
    let alive = true;
    const tick = () => {
      const r = run.current;
      const l = live.current;
      if (l.tab === null) return;
      const mode = githubSchedule({ inFlight: r.inFlight, shown: l.shown, focused: focused(), on: l.on, lastAsk: r.urgent ? Number.NEGATIVE_INFINITY : r.lastAsk, nextMs: r.nextMs, now: Date.now(), headMoved: r.asked && l.head !== r.answeredHead, asked: r.asked });
      if (mode === "none") return;
      const askedTab = l.tab;
      const askedHead = l.head;
      r.inFlight = true;
      r.urgent = false;
      readGithub(askedTab, mode === "ask")
        .then((reply) => {
          if (!alive || askedTab !== live.current.tab || run.current !== r) return;
          r.asked = true;
          r.nextMs = reply.next_ms;
          r.answeredHead = askedHead;
          setState({ tab: askedTab, reply });
        })
        .catch(() => {
          if (run.current === r) r.asked = true;
        })
        .finally(() => {
          if (run.current !== r) return;
          r.inFlight = false;
          r.lastAsk = askedAt(mode, r.lastAsk, Date.now());
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
    };
  }, []);

  const refresh = useCallback(() => {
    run.current.urgent = true;
  }, []);

  const mine = on && tab !== null && state.tab === tab;
  return { reply: mine ? state.reply : null, refresh };
}
