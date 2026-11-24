import { useCallback, useEffect, useRef, useState } from "react";
import type { GithubReply } from "../../generated/GithubReply";
import type { PullThreads } from "../../generated/PullThreads";
import type { ThreadsRequest } from "../../generated/ThreadsRequest";
import { failureOf, readThreads } from "./githubApi";

/** Whether to ask for the comments: the bar's reply names an open or draft pull request. */
export function threadsTarget(reply: GithubReply | null): ThreadsRequest | null {
  if (reply === null || reply.kind !== "ready" || reply.pull === null) return null;
  if (reply.pull.state !== "open" && reply.pull.state !== "draft") return null;
  return { checkout: reply.checkout, id: reply.pull.id, number: reply.pull.number };
}

/** Whether two requests name the same pull request of the same checkout. */
export const sameRequest = (a: ThreadsRequest, b: ThreadsRequest | null): boolean => b !== null && a.checkout === b.checkout && a.id === b.id && a.number === b.number;

/** Whether an answer is taken: it is of the latest ask (none started since, by a refresh or a new bar reply), and for the pull request the bar names now. */
export function takesAnswer(o: { started: number; latest: number; asked: ThreadsRequest; target: ThreadsRequest | null }): boolean {
  return o.started === o.latest && sameRequest(o.asked, o.target);
}

type State = { tab: string; key: string | null; threads: PullThreads | null; failed: string | null };

const keyOf = (t: ThreadsRequest | null): string | null => (t === null ? null : `${t.checkout}\u0000${t.id}\u0000${t.number}`);

/**
 * The comments of the bar's pull request. Asks once after each of the bar's own asks (`asked`) and after `refresh()`; drops an answer
 * that started before the last of either or is for another pull request; keeps the last comments while a read fails and gives the
 * failure's sentence. Null with no target.
 */
export function useThreads(tab: string, reply: GithubReply | null, asked: number): { threads: PullThreads | null; failed: string | null; refresh(): void } {
  const target = threadsTarget(reply);
  const key = keyOf(target);
  const [state, setState] = useState<State>({ tab, key: null, threads: null, failed: null });
  const [refreshes, setRefreshes] = useState(0);
  const live = useRef({ target, tab });
  live.current = { target, tab };
  const seq = useRef(0);

  useEffect(() => {
    const started = ++seq.current;
    if (target === null) {
      setState((s) => (s.key === null && s.threads === null && s.failed === null ? s : { tab, key: null, threads: null, failed: null }));
      return;
    }
    const now = () => takesAnswer({ started, latest: seq.current, asked: target, target: live.current.target }) && live.current.tab === tab;
    readThreads(tab, target).then(
      (threads) => {
        if (now()) setState({ tab, key, threads, failed: null });
      },
      (e) => {
        if (now()) setState((s) => ({ tab, key, threads: s.key === key && s.tab === tab ? s.threads : null, failed: failureOf(e).reason }));
      },
    );
    // The key stands for the target: a new reply for the same pull request asks again only through `asked` or `refreshes`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, key, asked, refreshes]);

  const refresh = useCallback(() => setRefreshes((n) => n + 1), []);
  const mine = state.tab === tab && state.key === key && key !== null;
  return { threads: mine ? state.threads : null, failed: mine ? state.failed : null, refresh };
}
