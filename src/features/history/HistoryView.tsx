import { Fragment, useEffect, useId, useRef, type KeyboardEvent } from "react";
import type { Conversation } from "../../generated/Conversation";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { SearchHit } from "../../generated/SearchHit";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { ConversationView } from "./ConversationView";
import { filterOptions, groupByProject, moveSelection, rowMeta } from "./historyState";
import { splitSnippet } from "./snippet";
import "./History.css";

export type When = "any" | "today" | "7d" | "30d";
export type FilterName = "agent" | "project" | "branch" | "model" | "when";
/** An unset filter is left out. */
export type HistoryFilters = { agent?: string; project?: string; branch?: string; model?: string; when: When };
/** Which conversation, and for a search hit the turn it is in. */
export type OpenTarget = { agent: string; session_id: string; turn?: number };

export type HistoryViewProps = {
  entries: HistoryEntry[];
  filters: HistoryFilters;
  query: string;
  /** The search results, or null when the list is showing instead (no query). */
  hits: SearchHit[] | null;
  /** A row's key: a session id in the list, `hitKey(hit)` among hits. */
  selectedId: string | null;
  /** The open conversation, "loading" while it is fetched, `{ error }` when it can't be shown, null when none is open. */
  conversation: Conversation | "loading" | { error: string } | null;
  /** The tab that holds this entry's session right now, if one does. */
  liveTabFor: (e: HistoryEntry) => string | null;
  now: number;
  os: Os;
  /** Each new value puts the cursor in the search box. */
  searchFocus: number;
  onQuery: (q: string) => void;
  /** An empty value clears the filter. */
  onFilter: (name: FilterName, value: string) => void;
  onSelect: (key: string) => void;
  onOpen: (target: OpenTarget) => void;
  onResume: (target: { agent: string; session_id: string }) => void;
  onGoTo: (tab: string) => void;
  onEscape: () => void;
};

/** A hit's row key: one session can match in several turns. */
export const hitKey = (h: Pick<SearchHit, "session_id" | "turn">) => `${h.session_id}:${h.turn}`;

const folderName = (cwd: string) => cwd.split(/[\\/]/).filter(Boolean).pop() ?? cwd;
const UNTITLED = "Untitled conversation";

const WHEN_LABELS: [When, string][] = [
  ["any", "Any time"],
  ["today", "Today"],
  ["7d", "Last 7 days"],
  ["30d", "Last 30 days"],
];

type Row =
  | { key: string; kind: "entry"; entry: HistoryEntry }
  | { key: string; kind: "hit"; hit: SearchHit };

/** The options of a filter: what the list holds, plus the chosen value even when nothing holds it any more. */
const withCurrent = (list: string[], current: string | undefined) =>
  current && !list.includes(current) ? [...list, current].sort((a, b) => a.localeCompare(b)) : list;

function Snippet({ text }: { text: string }) {
  return (
    <span className="history__snippet" dir="auto">
      {splitSnippet(text).map((s, i) => (s.hit ? <mark key={i}>{s.text}</mark> : <Fragment key={i}>{s.text}</Fragment>))}
    </span>
  );
}

/** The History view: a search box, filters and a grouped listbox of conversations beside the open conversation. Presentational; `LiveHistory` feeds it. */
export function HistoryView(p: HistoryViewProps) {
  const uid = useId();
  const searching = p.query.trim() !== "" && p.hits !== null;
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const sideRef = useRef<HTMLDivElement>(null);
  const detailRef = useRef<HTMLDivElement>(null);
  const rootRef = useRef<HTMLElement>(null);

  const groups = searching ? [] : groupByProject(p.entries);
  const rows: Row[] = searching
    ? p.hits!.map((hit) => ({ key: hitKey(hit), kind: "hit", hit }))
    : groups.flatMap((g) => g.entries.map((entry): Row => ({ key: entry.session_id, kind: "entry", entry })));
  const keys = rows.map((r) => r.key);
  const domId = (key: string) => `${uid}-opt-${keys.indexOf(key)}`;
  const selected = rows.find((r) => r.key === p.selectedId);

  useEffect(() => {
    searchRef.current?.focus();
    searchRef.current?.select();
  }, [p.searchFocus]);

  // In a narrow view the list is hidden while a conversation is open (and the conversation while none is): focus must follow,
  // or it falls to the page and Esc, which is handled here, never arrives.
  const open = p.conversation !== null;
  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    if (open && sideRef.current?.getClientRects().length === 0) detailRef.current?.focus();
    if (!open && !root.contains(document.activeElement)) (listRef.current ?? searchRef.current)?.focus();
  }, [open]);

  useEffect(() => {
    if (p.selectedId === null) return;
    document.getElementById(domId(p.selectedId))?.scrollIntoView?.({ block: "nearest" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.selectedId]);

  const openRow = (r: Row | undefined) => {
    if (!r) return;
    if (r.kind === "hit") p.onOpen({ agent: r.hit.agent, session_id: r.hit.session_id, turn: r.hit.turn });
    else p.onOpen({ agent: r.entry.agent, session_id: r.entry.session_id });
  };

  const onListKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "ArrowUp" || e.key === "ArrowDown" || e.key === "Home" || e.key === "End") {
      e.preventDefault();
      const next = moveSelection(keys, p.selectedId, e.key);
      if (next !== null) p.onSelect(next);
    } else if (e.key === "Enter") {
      e.preventDefault();
      openRow(selected);
    }
  };

  const onSearchKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && keys.length > 0) {
      e.preventDefault();
      listRef.current?.focus();
      const next = moveSelection(keys, p.selectedId, e.key);
      if (next !== null) p.onSelect(next);
    }
  };

  const onRootKey = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    e.preventDefault();
    p.onEscape();
  };

  const options = filterOptions(p.entries);
  const choose = (name: Exclude<FilterName, "when">, label: string, list: string[], display: (v: string) => string = (v) => v) => {
    const current = p.filters[name];
    return (
      <label className="history__filter">
        <span className="history__filter-label">{label}</span>
        <select className="history__select" value={current ?? ""} onChange={(e) => p.onFilter(name, e.target.value)}>
          <option value="">Any</option>
          {withCurrent(list, current).map((v) => (
            <option key={v} value={v} title={v}>
              {display(v)}
            </option>
          ))}
        </select>
      </label>
    );
  };

  const shown = p.conversation;
  const live = (c: Conversation) => {
    const e = p.entries.find((x) => x.agent === c.agent && x.session_id === c.session_id);
    return e ? p.liveTabFor(e) : null;
  };
  const shownConversation = typeof shown === "object" && shown !== null && "session_id" in shown ? shown : null;
  const scrollToTurn = selected?.kind === "hit" && shownConversation?.session_id === selected.hit.session_id ? selected.hit.turn : undefined;

  const searchHint = `Search conversations (${shortcutLabel(commandById("global-search")!.binding!, p.os)})`;

  return (
    <section ref={rootRef} className="history" aria-labelledby={`${uid}-title`} onKeyDown={onRootKey}>
      <div className="history__body" data-detail={open ? "open" : undefined}>
        <div ref={sideRef} className="history__side">
          <div className="history__head">
            <h2 className="history__title" id={`${uid}-title`}>
              History
            </h2>
            <input
              ref={searchRef}
              className="history__search"
              type="text"
              role="searchbox"
              aria-label="Search conversations"
              placeholder="Search conversations"
              title={searchHint}
              autoComplete="off"
              spellCheck={false}
              dir="auto"
              value={p.query}
              onChange={(e) => p.onQuery(e.target.value)}
              onKeyDown={onSearchKey}
            />
            <div className="history__filters">
              {choose("agent", "Agent", options.agents)}
              {choose("project", "Project", options.projects, folderName)}
              {choose("branch", "Branch", options.branches)}
              {choose("model", "Model", options.models)}
              <label className="history__filter">
                <span className="history__filter-label">When</span>
                <select className="history__select" value={p.filters.when} onChange={(e) => p.onFilter("when", e.target.value)}>
                  {WHEN_LABELS.map(([v, label]) => (
                    <option key={v} value={v}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </div>

          {rows.length === 0 ? (
            <p className="history__empty">
              {searching
                ? `Nothing matches “${p.query}”.`
                : "No conversations yet. Claude Code sessions you run in Marshell show up here when their first turn ends."}
            </p>
          ) : (
            <div
              ref={listRef}
              className="history__list"
              role="listbox"
              tabIndex={0}
              aria-label={searching ? "Search results" : "Conversations"}
              aria-activedescendant={selected ? domId(selected.key) : undefined}
              onKeyDown={onListKey}
            >
              {searching
                ? rows.map((r) =>
                    r.kind === "hit" ? (
                      <HitRow
                        key={r.key}
                        id={domId(r.key)}
                        hit={r.hit}
                        selected={r.key === p.selectedId}
                        onPick={() => {
                          p.onSelect(r.key);
                          openRow(r);
                        }}
                      />
                    ) : null,
                  )
                : groups.map((g, gi) => (
                    <div key={g.project} role="group" aria-labelledby={`${uid}-group-${gi}`}>
                      <h3 className="history__project" id={`${uid}-group-${gi}`} title={g.project} dir="auto">
                        {g.label}
                      </h3>
                      {g.entries.map((entry) => (
                        <div
                          key={entry.session_id}
                          id={domId(entry.session_id)}
                          role="option"
                          aria-selected={entry.session_id === p.selectedId}
                          className="history__row"
                          onClick={() => {
                            p.onSelect(entry.session_id);
                            p.onOpen({ agent: entry.agent, session_id: entry.session_id });
                          }}
                        >
                          <span className="history__row-title" dir="auto">
                            {entry.title || UNTITLED}
                          </span>
                          <span className="history__meta" dir="auto">
                            {rowMeta(entry, p.now)}
                          </span>
                        </div>
                      ))}
                    </div>
                  ))}
            </div>
          )}
        </div>

        <div ref={detailRef} className="history__detail" role="region" aria-label="Conversation" tabIndex={0}>
          {shown === null ? (
            <p className="history__empty">Choose a conversation to read it.</p>
          ) : shown === "loading" ? (
            <p className="history__empty">
              Loading conversation…
            </p>
          ) : "error" in shown ? (
            <p className="history__empty">
              {shown.error}
            </p>
          ) : (
            <ConversationView
              conversation={shown}
              scrollToTurn={scrollToTurn}
              liveTab={live(shown)}
              onResume={() => p.onResume({ agent: shown.agent, session_id: shown.session_id })}
              onGoTo={p.onGoTo}
            />
          )}
        </div>
      </div>
    </section>
  );
}

function HitRow({ id, hit, selected, onPick }: { id: string; hit: SearchHit; selected: boolean; onPick: () => void }) {
  return (
    <div id={id} role="option" aria-selected={selected} className="history__row" onClick={onPick}>
      <span className="history__row-title" dir="auto">
        {hit.title || UNTITLED}
      </span>
      <Snippet text={hit.snippet} />
      <span className="history__meta" dir="auto">{`${folderName(hit.cwd)} · turn ${hit.turn + 1}`}</span>
    </div>
  );
}
