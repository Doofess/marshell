import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import type { Conversation } from "../../generated/Conversation";
import type { Endpoint } from "../../generated/Endpoint";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { SearchHit } from "../../generated/SearchHit";
import { history } from "../../lib/api/history";
import { ApiError } from "../../lib/api/client";
import type { Os } from "../../lib/keymap";
import { WebLinkBar } from "../preview/WebLinkBar";
import type { RowModel } from "../sidebar/types";
import { NO_ARCHIVE_COPY } from "./ConversationView";
import {
  EMPTY_LIST_COPY,
  HistoryView,
  noHitsCopy,
  type FilterName,
  type HistoryFilters,
  type HistoryViewProps,
  type When,
} from "./HistoryView";
import { escapeAction, sinceFor } from "./historyState";

/** How long the search waits after the last keystroke. */
const SEARCH_DELAY_MS = 150;
const LOADING_COPY = "Loading conversations…";
export const NO_FILTER_MATCH_COPY = "No conversations match these filters.";

/** What the list says when it has no rows and no search is running: the failure, loading, or why it is empty (a filter that excludes everything is not "no conversations yet"). */
export function listEmptyText(filters: HistoryFilters, loaded: boolean, error: string | null): string {
  if (error) return error;
  if (!loaded) return LOADING_COPY;
  const filtered = filters.when !== "any" || Boolean(filters.agent || filters.project || filters.branch || filters.model);
  return filtered ? NO_FILTER_MATCH_COPY : EMPTY_LIST_COPY;
}

/** The tab that holds an entry's session right now: a row of that tab whose state isn't Ended (an Ended tab has no session to go to). */
export const liveTabFor =
  (rows: RowModel[]) =>
  (e: Pick<HistoryEntry, "tab_id">): string | null =>
    rows.find((r) => r.id === e.tab_id && r.status !== "ended")?.id ?? null;

/** What a failed request says: the core's words, or the error's. */
export function failureText(e: unknown): string {
  if (e instanceof ApiError) return e.message.trim() || `The core answered ${e.status}.`;
  return e instanceof Error && e.message ? e.message : String(e);
}

/** What the conversation pane says when a conversation can't be shown. A 404 with the no-archive message gets the view's own copy. */
export function conversationFailure(e: unknown): string {
  if (e instanceof ApiError && e.status === 404 && e.message.trim() === NO_ARCHIVE_COPY) return NO_ARCHIVE_COPY;
  return failureText(e);
}

type Listed = { entries: HistoryEntry[]; now: number; loaded: boolean; error: string | null };
type Searched = { query: string; hits: SearchHit[]; error: string | null };
type Shown = Conversation | "loading" | { error: string } | null;
type Link = Parameters<NonNullable<HistoryViewProps["onLink"]>>[0];

export type LiveHistoryProps = {
  endpoint: Endpoint;
  /** The window's sessions: an entry whose tab is a live row among them offers "Go to session". */
  rows: RowModel[];
  os: Os;
  /** Each new value puts the cursor in the search box. */
  searchFocus: number;
  /** The size a resumed session starts at. */
  size: () => { cols: number; rows: number };
  /** Says something through the window's announcer. */
  announce: (text: string) => void;
  onGoTo: (tabId: string) => void;
  /** The conversation was resumed in this new tab. */
  onResumed: (tabId: string) => void;
  /** A file link in assistant text: the window opens it in the preview, as the preview's own links do. */
  onOpenFile: (link: { path: string; line?: number }) => void;
  onClose: () => void;
};

/** The History view wired to the core: it fetches the list on open and on each filter change, searches 150 ms after the last keystroke, opens a conversation on Enter or a click, and resumes it. */
export function LiveHistory({ endpoint, rows, os, searchFocus, size, announce, onGoTo, onResumed, onOpenFile, onClose }: LiveHistoryProps) {
  const [filters, setFilters] = useState<HistoryFilters>({ when: "any" });
  const [listed, setListed] = useState<Listed>({ entries: [], now: Date.now(), loaded: false, error: null });
  const [query, setQuery] = useState("");
  const [searched, setSearched] = useState<Searched | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [conversation, setConversation] = useState<Shown>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [web, setWeb] = useState<string | null>(null);
  const openSeq = useRef(0);
  const resuming = useRef(false);
  const returnTo = useRef<HTMLElement | null>(null);

  // The list: again on each filter change. A response that comes back after a newer request began is dropped; "now" is when it was asked.
  useEffect(() => {
    let live = true;
    const now = Date.now();
    const { when, ...rest } = filters;
    history.list(endpoint, { ...rest, since_ms: sinceFor(when, now) }).then(
      (entries) => live && setListed({ entries, now, loaded: true, error: null }),
      (e: unknown) => live && setListed({ entries: [], now, loaded: true, error: failureText(e) }),
    );
    return () => {
      live = false;
    };
  }, [endpoint, filters]);

  // The search: 150 ms after the last keystroke; an empty query shows the list again. Closing the view clears the pending timer.
  useEffect(() => {
    const q = query.trim();
    if (q === "") {
      setSearched(null);
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      history.search(endpoint, q).then(
        (hits) => live && setSearched({ query: q, hits, error: null }),
        (e: unknown) => live && setSearched({ query: q, hits: [], error: failureText(e) }),
      );
    }, SEARCH_DELAY_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [endpoint, query]);

  const searching = query.trim() !== "" && searched !== null;
  const hits = searching ? searched.hits : null;
  const emptyText = searching ? (searched.error ?? noHitsCopy(searched.query)) : listEmptyText(filters, listed.loaded, listed.error);
  const isEmpty = searching ? searched.hits.length === 0 : listed.loaded && listed.entries.length === 0;

  // What the list says when it has nothing is spoken once when it appears or changes, not on every render.
  const emptySpoken = isEmpty ? emptyText : null;
  useEffect(() => {
    if (emptySpoken) announce(emptySpoken);
  }, [emptySpoken, announce]);
  const failed = typeof conversation === "object" && conversation !== null && "error" in conversation ? conversation.error : null;
  useEffect(() => {
    if (failed) announce(failed);
  }, [failed, announce]);

  const open = (target: { agent: string; session_id: string }) => {
    const seq = ++openSeq.current;
    setRefusal(null);
    setWeb(null);
    setConversation("loading");
    history.get(endpoint, target.agent, target.session_id).then(
      (c) => seq === openSeq.current && setConversation(c),
      (e: unknown) => seq === openSeq.current && setConversation({ error: conversationFailure(e) }),
    );
  };

  const resume = async (target: { agent: string; session_id: string }) => {
    if (resuming.current) return;
    resuming.current = true;
    setRefusal(null);
    try {
      const r = await history.resume(endpoint, target.agent, target.session_id, size());
      if (r.ok) return onResumed(r.tabId);
      setRefusal(r.reason);
      announce(r.reason);
    } catch (e) {
      const text = `Couldn't resume: ${failureText(e)}`;
      setRefusal(text);
      announce(text);
    } finally {
      resuming.current = false;
    }
  };

  const escape = () => {
    const action = escapeAction({ conversationOpen: conversation !== null, query });
    if (action === "close-conversation") {
      openSeq.current++;
      setConversation(null);
      setRefusal(null);
    } else if (action === "clear-search") setQuery("");
    else onClose();
  };

  const filter = (name: FilterName, value: string) =>
    setFilters((f) => {
      if (name === "when") return { ...f, when: value as When };
      const next = { ...f };
      if (value === "") delete next[name];
      else next[name] = value;
      return next;
    });

  // A web link asks first, like the preview's; a file link is the window's to open.
  const follow = (link: Link) => {
    if (link.kind === "web") {
      returnTo.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setWeb(link.url);
    } else onOpenFile(link.line === undefined ? { path: link.path } : { path: link.path, line: link.line });
  };
  const answer = (yes: boolean) => {
    if (yes && web) void openUrl(web).catch(() => {});
    setWeb(null);
    const back = returnTo.current;
    (back?.isConnected ? back : document.querySelector<HTMLElement>(".history__detail"))?.focus();
  };

  const banner =
    refusal || web ? (
      <>
        {refusal && <p className="history__refusal">{refusal}</p>}
        {web && <WebLinkBar url={web} onOpen={() => answer(true)} onCancel={() => answer(false)} />}
      </>
    ) : undefined;

  return (
    <HistoryView
      entries={listed.entries}
      filters={filters}
      query={query}
      hits={hits}
      selectedId={selectedId}
      conversation={conversation}
      liveTabFor={liveTabFor(rows)}
      now={listed.now}
      os={os}
      searchFocus={searchFocus}
      onQuery={setQuery}
      onFilter={filter}
      onSelect={setSelectedId}
      onOpen={open}
      onResume={(t) => void resume(t)}
      onGoTo={onGoTo}
      onEscape={escape}
      emptyText={emptyText}
      banner={banner}
      onLink={follow}
    />
  );
}
