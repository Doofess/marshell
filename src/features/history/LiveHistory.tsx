import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import type { Conversation } from "../../generated/Conversation";
import type { Endpoint } from "../../generated/Endpoint";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { ImportStatus } from "../../generated/ImportStatus";
import type { SearchHit } from "../../generated/SearchHit";
import type { HistoryPatch } from "../../generated/HistoryPatch";
import type { RecordingPart } from "../../generated/RecordingPart";
import { history, saveConversation } from "../../lib/api/history";
import { ApiError } from "../../lib/api/client";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { WebLinkBar } from "../preview/WebLinkBar";
import type { RowModel } from "../sidebar/types";
import { NO_ARCHIVE_COPY, type ConversationEditor, type ConversationTab } from "./ConversationView";
import { copyExport, ExportDialog } from "./ExportDialog";
import { actionAnnouncement, deleteConversation, forkConversation } from "./historyActions";
import {
  EMPTY_LIST_COPY,
  HistoryView,
  noBookmarksCopy,
  noHitsCopy,
  type FilterName,
  type HistoryFilters,
  type HistoryViewProps,
  type When,
} from "./HistoryView";
import { bookmarkQuery, escapeAction, importLine, listQuery } from "./historyState";
import { RecordingTab, type PlayRequest, type RecordingState } from "./recording/RecordingTab";
import type { TerminalThemeProp } from "../terminal/TerminalView";

/** How long the search waits after the last keystroke. */
const SEARCH_DELAY_MS = 150;
const IMPORT_POLL_MS = 1000;
const IMPORT_DONE_COPY = "Finished copying conversations from Claude Code and Codex.";
const LOADING_COPY = "Loading conversations…";
export const NO_FILTER_MATCH_COPY = "No conversations match these filters.";

/** What the list says when it has no rows and no search is running: the failure, loading, or why it is empty (a filter that excludes everything is not "no conversations yet"). */
export function listEmptyText(filters: HistoryFilters, loaded: boolean, error: string | null, bookmarkShortcut = ""): string {
  if (error) return error;
  if (!loaded) return LOADING_COPY;
  if (filters.bookmarked) return noBookmarksCopy(bookmarkShortcut);
  const filtered = filters.when !== "any" || filters.place !== "any" || Boolean(filters.agent || filters.project || filters.branch || filters.model || filters.tag);
  return filtered ? NO_FILTER_MATCH_COPY : EMPTY_LIST_COPY;
}

/** The turns (0-based) "Play from here" can start: a turn with a time that falls inside one of the recording's parts. */
export function playableTurns(parts: Pick<RecordingPart, "started_ms" | "ended_ms">[], turns: { index: number; at_ms: number }[]): Set<number> {
  const out = new Set<number>();
  for (const t of turns) if (t.at_ms > 0 && parts.some((p) => t.at_ms >= p.started_ms && t.at_ms <= p.ended_ms)) out.add(t.index);
  return out;
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
type Marked = { hits: SearchHit[]; error: string | null };
type Searched = { query: string; hits: SearchHit[]; error: string | null };
type Shown = Conversation | "loading" | { error: string } | null;
type Opened = { agent: string; session_id: string };
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
  /** The live terminals' look, which the recording player shares. */
  theme?: TerminalThemeProp;
  font?: { family: string; size: number };
};

/** The History view wired to the core: it fetches the list on open and on each filter change, searches 150 ms after the last keystroke, opens a conversation on Enter or a click, and resumes it. */
export function LiveHistory({ endpoint, rows, os, searchFocus, size, announce, onGoTo, onResumed, onOpenFile, onClose, theme, font }: LiveHistoryProps) {
  const [filters, setFilters] = useState<HistoryFilters>({ when: "any", bookmarked: false, archived: false, place: "any", headless: false });
  const [importState, setImportState] = useState<ImportStatus | null>(null);
  const [listed, setListed] = useState<Listed>({ entries: [], now: Date.now(), loaded: false, error: null });
  const [query, setQuery] = useState("");
  const [searched, setSearched] = useState<Searched | null>(null);
  const [marked, setMarked] = useState<Marked | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [conversation, setConversation] = useState<Shown>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [web, setWeb] = useState<string | null>(null);
  const openSeq = useRef(0);
  const resuming = useRef(false);
  const returnTo = useRef<HTMLElement | null>(null);
  const [reload, setReload] = useState(0);
  const [editor, setEditor] = useState<ConversationEditor | null>(null);
  const [editorError, setEditorError] = useState<string | undefined>(undefined);
  const [exporting, setExporting] = useState<{ target: Opened; title: string } | null>(null);
  const shown = typeof conversation === "object" && conversation !== null && "turns" in conversation ? conversation : null;
  const shownRef = useRef(shown);
  shownRef.current = shown;
  const [tab, setTab] = useState<ConversationTab>("conversation");
  const [request, setRequest] = useState<PlayRequest | null>(null);
  const [parts, setParts] = useState<RecordingState>({ status: "loading" });

  // The recording's parts load beside the conversation, once per conversation (a refresh after a bookmark keeps them).
  const shownAgent = shown?.agent;
  const shownId = shown?.session_id;
  useEffect(() => {
    if (shownAgent === undefined || shownId === undefined) return;
    let live = true;
    setParts({ status: "loading" });
    history.recordingParts(endpoint, shownAgent, shownId).then(
      (list) => live && setParts({ status: "ready", parts: list }),
      (e: unknown) => live && setParts({ status: "failed", reason: failureText(e) }),
    );
    return () => {
      live = false;
    };
  }, [endpoint, shownAgent, shownId]);

  // Loads the list and the open conversation again: after a change, and when a session is renamed (a conversation shares its tab's name).
  const refresh = () => {
    setReload((n) => n + 1);
    const c = shownRef.current;
    if (!c) return;
    const seq = ++openSeq.current;
    history.get(endpoint, c.agent, c.session_id).then(
      (next) => seq === openSeq.current && setConversation(next),
      () => {},
    );
  };
  const names = rows.map((r) => r.name).join("\u0000");
  const lastNames = useRef(names);
  useEffect(() => {
    if (lastNames.current === names) return;
    lastNames.current = names;
    refresh();
  }, [names]);

  // The list: again on each filter change. A response that comes back after a newer request began is dropped; "now" is when it was asked.
  useEffect(() => {
    let live = true;
    const now = Date.now();
    history.list(endpoint, listQuery(filters, now)).then(
      (entries) => live && setListed({ entries, now, loaded: true, error: null }),
      (e: unknown) => live && setListed({ entries: [], now, loaded: true, error: failureText(e) }),
    );
    return () => {
      live = false;
    };
  }, [endpoint, filters, reload]);

  // The import's progress: read once when History opens, then every second for as long as the last answer said it was copying.
  // The line is spoken when copying starts and when it ends, not on each count; the list loads again once when it ends.
  useEffect(() => {
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let wasCopying = false;
    const tick = async () => {
      let next: ImportStatus | null = null;
      try {
        next = await history.importStatus(endpoint);
      } catch {
        // A failed read ends the polling; reopening History reads again.
      }
      if (!live) return;
      const copying = next?.copying === true;
      if (copying && !wasCopying) announce(importLine(next) ?? "");
      if (!copying && wasCopying) {
        announce(IMPORT_DONE_COPY);
        setReload((n) => n + 1);
      }
      wasCopying = copying;
      setImportState(next);
      if (copying) timer = setTimeout(() => void tick(), IMPORT_POLL_MS);
    };
    void tick();
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [endpoint, announce]);

  // The bookmarks: one row per bookmark, fetched while the Bookmarked filter is on.
  useEffect(() => {
    setMarked(null);
    if (!filters.bookmarked) return;
    let live = true;
    history.bookmarks(endpoint, bookmarkQuery(filters, Date.now())).then(
      (hits) => live && setMarked({ hits, error: null }),
      (e: unknown) => live && setMarked({ hits: [], error: failureText(e) }),
    );
    return () => {
      live = false;
    };
  }, [endpoint, filters, reload]);

  // The search: 150 ms after the last keystroke; an empty query shows the list again. Closing the view clears the pending timer.
  useEffect(() => {
    const q = query.trim();
    if (q === "") {
      setSearched(null);
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      history.search(endpoint, q, filters.archived, filters.headless).then(
        (hits) => live && setSearched({ query: q, hits, error: null }),
        (e: unknown) => live && setSearched({ query: q, hits: [], error: failureText(e) }),
      );
    }, SEARCH_DELAY_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [endpoint, query, filters.archived, filters.headless, reload]);

  const searching = query.trim() !== "" && searched !== null;
  const showMarked = !searching && query.trim() === "" && filters.bookmarked;
  const bookmarkShortcut = shortcutLabel(commandById("bookmark")!.binding!, os);
  const hits = searching ? searched.hits : showMarked ? (marked?.hits ?? []) : null;
  const emptyText = searching
    ? (searched.error ?? noHitsCopy(searched.query))
    : showMarked
      ? marked === null
        ? LOADING_COPY
        : (marked.error ?? noBookmarksCopy(bookmarkShortcut))
      : listEmptyText(filters, listed.loaded, listed.error, bookmarkShortcut);
  const isEmpty = searching ? searched.hits.length === 0 : showMarked ? marked !== null && marked.hits.length === 0 : listed.loaded && listed.entries.length === 0;

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
    setTab("conversation");
    setRequest(null);
    setEditor(null);
    setEditorError(undefined);
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

  const fork = async (target: Opened) => {
    if (resuming.current) return;
    resuming.current = true;
    setRefusal(null);
    try {
      await forkConversation(endpoint, target, size(), {
        onResumed,
        refuse: (text) => {
          setRefusal(text);
          announce(text);
        },
      });
    } finally {
      resuming.current = false;
    }
  };

  // A change that failed shows under the open editor, or in the banner when none is open.
  const fail = (text: string) => {
    if (editor !== null) setEditorError(text);
    else setRefusal(text);
    announce(text);
  };

  const patch = async (p: HistoryPatch) => {
    const c = shown;
    if (!c) return;
    try {
      const r = await history.patch(endpoint, c.agent, c.session_id, p);
      if (!r.ok) return fail(r.reason);
      setEditorError(undefined);
      setEditor(null);
      announce(actionAnnouncement(p, r.entry));
      refresh();
    } catch (e) {
      fail(`Couldn't save: ${failureText(e)}`);
    }
  };

  const bookmarkTurn = async (turn: number, on: boolean) => {
    const c = shown;
    if (!c) return;
    try {
      const r = await history.bookmarkTurn(endpoint, c.agent, c.session_id, turn, on);
      if (!r.ok) return fail(r.reason);
      refresh();
    } catch (e) {
      fail(`Couldn't bookmark: ${failureText(e)}`);
    }
  };

  const copyMarkdown = async () => {
    const c = shown;
    if (!c) return;
    try {
      const text = await history.exportText(endpoint, c.agent, c.session_id, "markdown", true);
      await copyExport({ text, format: "markdown", onCopy: (t) => navigator.clipboard.writeText(t), announce });
    } catch (e) {
      fail(`Couldn't copy: ${failureText(e)}`);
    }
  };

  const remove = async () => {
    const c = shown;
    if (!c) return;
    await deleteConversation(endpoint, c, c.title || "Untitled conversation", {
      announce,
      closed: () => {
        openSeq.current++;
        setConversation(null);
        setSelectedId(null);
        setRefusal(null);
        setEditor(null);
        setEditorError(undefined);
      },
      focusList: () =>
        setTimeout(() => (document.querySelector<HTMLElement>('.history [role="listbox"]') ?? document.querySelector<HTMLElement>(".history__search"))?.focus(), 0),
      refresh: () => setReload((n) => n + 1),
      refuse: fail,
    });
  };

  const escape = () => {
    const action = escapeAction({ conversationOpen: conversation !== null, query });
    if (action === "close-conversation") {
      openSeq.current++;
      setConversation(null);
      setRefusal(null);
      setEditor(null);
      setEditorError(undefined);
    } else if (action === "clear-search") setQuery("");
    else onClose();
  };

  const toggle = (name: "bookmarked" | "archived" | "headless", on: boolean) => setFilters((f) => ({ ...f, [name]: on }));

  const filter = (name: FilterName, value: string) =>
    setFilters((f) => {
      if (name === "when") return { ...f, when: value as When };
      if (name === "place") return { ...f, place: value as HistoryFilters["place"] };
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

  const view = (
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
      onToggle={toggle}
      onSelect={setSelectedId}
      onOpen={open}
      onResume={(t) => void resume(t)}
      onGoTo={onGoTo}
      onEscape={escape}
      emptyText={emptyText}
      importLine={importLine(importState)}
      banner={banner}
      onLink={follow}
      onFork={() => shown && void fork(shown)}
      onPatch={(p) => void patch(p)}
      onBookmark={(turn, on) => void bookmarkTurn(turn, on)}
      onExport={() => shown && setExporting({ target: shown, title: shown.title })}
      onCopyMarkdown={() => void copyMarkdown()}
      onDelete={() => void remove()}
      editor={editor}
      onEditor={(e) => {
        setEditor(e);
        setEditorError(undefined);
      }}
      error={editorError}
      tab={tab}
      onTab={(t) => {
        setTab(t);
        setRequest(null);
      }}
      playableTurns={shown && parts.status === "ready" ? playableTurns(parts.parts, shown.turns) : undefined}
      onPlayFrom={(turn) => {
        const at = shown?.turns.find((t) => t.index === turn)?.at_ms ?? 0;
        setRequest({ turn, atMs: at });
        setTab("recording");
      }}
      recording={
        shown ? (
          <RecordingTab
            agent={shown.agent}
            id={shown.session_id}
            parts={parts}
            turnTimes={shown.turns.map((t) => t.at_ms)}
            request={request}
            frames={(part, q) => history.recordingFrames(endpoint, shown.agent, shown.session_id, part, q)}
            theme={theme}
            font={font}
            platform={endpoint.platform}
          />
        ) : undefined
      }
    />
  );
  return (
    <>
      {view}
      {exporting && (
        <ExportDialog
          title={exporting.title}
          load={(format, redact) => history.exportText(endpoint, exporting.target.agent, exporting.target.session_id, format, redact)}
          onSave={saveConversation}
          onCopy={(text) => navigator.clipboard.writeText(text)}
          onClose={() => setExporting(null)}
          announce={announce}
        />
      )}
    </>
  );
}
