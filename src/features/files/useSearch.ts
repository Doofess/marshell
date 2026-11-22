import { useEffect, useRef, useState } from "react";
import type { SearchReply } from "../../generated/SearchReply";
import type { searchFiles } from "./filesApi";

export const SEARCH_DEBOUNCE_MS = 300;

export type SearchState = { reply: SearchReply | null; busy: boolean; failed: boolean };
const IDLE: SearchState = { reply: null, busy: false, failed: false };

export type SearchScheduler = { schedule(tab: string | null, query: string | null): void; dispose(): void };

/**
 * The timing and the aborting, with no React: a new query cancels the waiting one and aborts the running one, a reply for a
 * query that is no longer current is dropped, an aborted fetch is not a failure, and nothing is set after `dispose`.
 */
export function createSearchScheduler(run: typeof searchFiles, set: (s: SearchState) => void): SearchScheduler {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let ctl: AbortController | null = null;
  let current = 0;
  let last: SearchState = IDLE;
  let dead = false;
  const emit = (s: SearchState) => {
    last = s;
    if (!dead) set(s);
  };
  const cancel = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    ctl?.abort();
    ctl = null;
  };
  return {
    schedule(tab, query) {
      cancel();
      const mine = ++current;
      if (tab === null || query === null) {
        emit(IDLE);
        return;
      }
      emit({ reply: last.reply, busy: true, failed: false });
      timer = setTimeout(() => {
        timer = null;
        const c = new AbortController();
        ctl = c;
        run(tab, query, c.signal).then(
          (r) => {
            if (dead || mine !== current || c.signal.aborted) return;
            emit({ reply: r === "no-folder" ? null : r, busy: false, failed: false });
          },
          () => {
            if (dead || mine !== current || c.signal.aborted) return;
            emit({ reply: null, busy: false, failed: true });
          },
        );
      }, SEARCH_DEBOUNCE_MS);
    },
    dispose() {
      dead = true;
      cancel();
    },
  };
}

/** The content search for `query` in the session's project, run again when the project's `root` moves; a null `query` is no search. */
export function useSearch(o: { tab: string | null; root: string | null; query: string | null; run: typeof searchFiles }): SearchState {
  const [state, setState] = useState<SearchState>(IDLE);
  const scheduler = useRef<SearchScheduler | null>(null);
  useEffect(() => {
    const s = createSearchScheduler(o.run, setState);
    scheduler.current = s;
    return () => {
      s.dispose();
      scheduler.current = null;
    };
  }, [o.run]);
  useEffect(() => {
    scheduler.current?.schedule(o.tab, o.query);
  }, [o.tab, o.root, o.query, o.run]);
  return state;
}
