import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import { COMMANDS, commandById, shortcutLabel, type Command, type Os } from "../../lib/keymap";
import { usePopupFocus } from "../../lib/usePopupFocus";
import { filterCommands, filterSessions, highlight, parseSessionNumber, sessionDetail, type Hit, type Range, type SessionHit } from "../launcher/filter";
import { rankFiles, splitPath, type FileHit } from "../preview/quickOpen";
import type { PaletteSession } from "./fixtures";
import "./Palette.css";

export type PaletteView = { query: string; activeIndex: number };
export type { PaletteSession };

/**
 * A press anywhere in the palette but the box keeps the keys in the box: a focused heading or the page body would take Esc and the
 * arrows away, leaving the palette open over a window that hears the keys instead. A press on the dimmed window around the sheet closes it.
 */
export function pressInPalette(e: { target: unknown; preventDefault: () => void }, onClose?: () => void) {
  const at = (sel: string) => (e.target as { closest?: (s: string) => unknown } | null)?.closest?.(sel);
  if (at(".palette__input")) return;
  e.preventDefault();
  if (at(".palette__backdrop")) onClose?.();
}

/** The list scrolls inside the sheet's cap, so the row the arrows chose is kept in sight by hand. */
export function keepActiveInView(list: { querySelector: (sel: string) => { scrollIntoView?: (o: ScrollIntoViewOptions) => void } | null } | null, id: string) {
  list?.querySelector(`[id="${id}"]`)?.scrollIntoView?.({ block: "nearest" });
}

const GROUPS: Command["group"][] = ["Sessions", "View", "Tools"];
const FOCUS_SESSION = /^focus-session-\d+$/;

/** The files quick open offers, as display paths (`displayPath`): the ones seen in the session's output, and its folder's list. */
export type QuickFiles = {
  seen: string[];
  list: string[];
  folder: string;
  truncated?: boolean;
  /** The folder's list: still being fetched, failed, or here. Left out means ready. The seen paths do not wait for it. */
  status?: "loading" | "error" | "ready";
  /** "Open file…": file groups only. */
  only?: boolean;
};

/** Files shown per group. */
const FILE_LIMIT = 20;

type Item = { type: "session"; hit: SessionHit<PaletteSession> } | { type: "command"; hit: Hit<Command> } | { type: "file"; hit: FileHit; group: string };
const itemId = (i: Item) => (i.type === "session" ? `session:${i.hit.item.id}` : i.type === "file" ? `file:${i.hit.path}` : i.hit.item.id);

const Marked = ({ text, ranges }: { text: string; ranges: Range[] }) => (
  <>
    {highlight(text, ranges).map((s, i) =>
      s.hit ? (
        <mark key={i} dir="auto">
          {s.text}
        </mark>
      ) : (
        <span key={i}>{s.text}</span>
      ),
    )}
  </>
);

type OptionProps = { id: string; active: boolean; os: Os; onRun?: (id: string) => void };

function CommandOption({ hit, id, active, os, onRun }: OptionProps & { hit: Hit<Command> }) {
  const c = hit.item;
  return (
    <li id={id} role="option" {...selectedProps(active, "selected", "palette__opt")} data-active={active || undefined} onClick={() => onRun?.(c.id)}>
      <span className="palette__label" dir="auto">
        <Marked text={c.label} ranges={hit.ranges} />
      </span>
      {c.binding && <kbd className="palette__kbd">{shortcutLabel(c.binding, os)}</kbd>}
    </li>
  );
}

/** One open session: its number first, then its mark, name and where it runs. Sessions 1 to 9 show the Ctrl+digit that goes there. */
function SessionOption({ hit, id, active, os, onRun }: OptionProps & { hit: SessionHit<PaletteSession> }) {
  const s = hit.item;
  const detail = sessionDetail(s);
  // Only a whole number from 1 to 9 has a Ctrl+digit command; 0, negatives, fractions and 10 and up are reached by typing their number.
  const key = Number.isInteger(s.number) && s.number >= 1 && s.number <= 9 ? commandById(`focus-session-${s.number}`).binding : undefined;
  return (
    <li
      id={id}
      role="option"
      {...selectedProps(active, "selected", "palette__opt")}
      aria-label={`Go to session ${s.number}, ${s.name} ${detail}${key ? ` ${shortcutLabel(key, os)}` : ""}, ${GLYPHS[s.status].label}`}
      data-kind="session"
      data-active={active || undefined}
      data-exact={hit.exact || undefined}
      onClick={() => onRun?.(`session:${s.id}`)}
    >
      <span className="palette__num">{s.number}</span>{" "}
      <span className="palette__mark" aria-hidden="true">
        <AgentMark agent={s.kind} size={16} />
      </span>
      <span className="palette__label" dir="auto">
        <span className="palette__name">
          <Marked text={s.name} ranges={hit.ranges} />
        </span>{" "}
        <span className="palette__detail">
          <Marked text={detail} ranges={hit.detailRanges} />
        </span>
      </span>
      <span className="palette__status" aria-hidden="true">
        <StatusGlyph kind={s.status} />
      </span>
      {key && " "}
      {key && <kbd className="palette__kbd">{shortcutLabel(key, os)}</kbd>}
    </li>
  );
}

/** One file: its name, then the folder it is in. The ranges are over the whole path, so they are split at the name. */
function FileOption({ hit, id, active, onRun }: OptionProps & { hit: FileHit }) {
  const { name, dir } = splitPath(hit.path);
  const cut = Array.from(dir === "" ? "" : `${dir}/`).length;
  const inName = hit.ranges.flatMap(([a, b]): Range[] => (b > cut ? [[Math.max(a, cut) - cut, b - cut]] : []));
  const inDir = hit.ranges.flatMap(([a, b]): Range[] => (a < cut - 1 ? [[a, Math.min(b, cut - 1)]] : []));
  return (
    <li id={id} role="option" {...selectedProps(active, "selected", "palette__opt")} aria-label={dir === "" ? name : `${name}, ${dir}`} data-kind="file" data-active={active || undefined} onClick={() => onRun?.(`file:${hit.path}`)}>
      <span className="palette__label" dir="auto">
        <span className="palette__name">
          <Marked text={name} ranges={inName} />
        </span>{" "}
        {dir !== "" && (
          <span className="palette__detail">
            <Marked text={dir} ranges={inDir} />
          </span>
        )}
      </span>
    </li>
  );
}

/**
 * The command palette. Every action in the app is here, with its shortcut shown; shortcuts are rebindable in Settings.
 * With `sessions` it is also where you go to a session: each open one is a row with its number, and typing 6, #6 or
 * "session 6" puts that session first (the nine "Go to session N" commands give way to the rows, which show the same
 * Ctrl+digit). Presentational: the parent owns the query and the active row.
 */
export function Palette({
  view,
  sessions,
  commands = COMMANDS,
  os = "windows",
  files,
  onQuery,
  onRun,
  onActive,
  onClose,
}: {
  view: PaletteView;
  sessions?: PaletteSession[];
  /** The commands to list; all of them when left out. A window that runs only some of them passes just those. */
  commands?: Command[];
  os?: Os;
  /** Quick open: with 2 or more characters typed the palette also lists these; with `only` it lists nothing else. */
  files?: QuickFiles;
  onQuery?: (q: string) => void;
  /** Receives a command's id, `session:<id>` for a session row, or `file:<display path>` for a file. */
  onRun?: (id: string) => void;
  onActive?: (index: number) => void;
  onClose?: () => void;
}) {
  const { query } = view;
  // Ids are per palette (the review page shows many at once), so two never share an id.
  const uid = useId();
  const optId = (i: number) => `${uid}-opt-${i}`;
  const listId = `${uid}-list`;
  const list = useRef<HTMLUListElement>(null);
  const sheet = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const filtering = query.trim() !== "";
  const wanted = sessions ? parseSessionNumber(query) : null;

  const only = files?.only === true;
  const sessionHits = sessions && !only ? filterSessions(query, sessions) : [];
  const pool = sessions ? commands.filter((c) => !FOCUS_SESSION.test(c.id)) : commands;
  // A number means a session: no command should answer to "4" because its label says "up to 4".
  const commandHits = only || wanted !== null ? [] : filterCommands(query, pool);
  // One character matches too much to be worth a group; "Open file…" starts at one, and shows the seen paths before any.
  const fileGroups: Array<{ title: string; hits: FileHit[] }> = [];
  if (files && (only || query.trim().length >= 2)) {
    // A seen path leaves the Files group only once that group exists.
    const seenSet = new Set(files.status === undefined || files.status === "ready" ? files.seen : []);
    const seenHits = rankFiles(query, [], files.seen, FILE_LIMIT);
    const listHits = query.trim() === "" ? [] : rankFiles(query, files.list.filter((p) => !seenSet.has(p)), [], FILE_LIMIT);
    if (seenHits.length > 0) fileGroups.push({ title: "Seen in output", hits: seenHits });
    if (listHits.length > 0) fileGroups.push({ title: `Files in ${files.folder}`, hits: listHits });
  }
  const items: Item[] = [
    ...sessionHits.map((hit): Item => ({ type: "session", hit })),
    ...commandHits.map((hit): Item => ({ type: "command", hit })),
    ...fileGroups.flatMap((g) => g.hits.map((hit): Item => ({ type: "file", hit, group: g.title }))),
  ];

  // A stale or odd active index (the list got shorter, a NaN) falls back to a row that exists.
  const activeIndex = items.length === 0 ? 0 : Number.isFinite(view.activeIndex) ? Math.min(Math.max(0, Math.floor(view.activeIndex)), items.length - 1) : 0;

  const option = (item: Item, index: number) =>
    item.type === "session" ? (
      <SessionOption key={itemId(item)} hit={item.hit} id={optId(index)} active={index === activeIndex} os={os} onRun={onRun} />
    ) : item.type === "file" ? (
      <FileOption key={itemId(item)} hit={item.hit} id={optId(index)} active={index === activeIndex} os={os} onRun={onRun} />
    ) : (
      <CommandOption key={itemId(item)} hit={item.hit} id={optId(index)} active={index === activeIndex} os={os} onRun={onRun} />
    );

  const numbers = sessions ? [...sessions].map((s) => s.number).sort((a, b) => a - b) : [];
  const empty =
    only
      ? files?.status === "loading"
        ? "Listing files…"
        : files?.status === "error"
        ? files.folder
          ? `Can't list the files in ${files.folder}.`
          : "Can't list files for this session."
        : query.trim() === ""
        ? "Type a file name"
        : `No file matches “${query}”`
      : wanted !== null
      ? `No session ${wanted}. ${numbers.length > 0 ? `Open sessions: ${numbers.join(", ")}.` : "No sessions are open."}`
      : `${sessions ? "No session or command matches" : "No command matches"} “${query}”`;

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose?.();
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && items.length > 0) {
      e.preventDefault();
      onActive?.((activeIndex + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length);
    } else if (e.key === "Enter") {
      const item = items[activeIndex];
      if (item) {
        e.preventDefault();
        onRun?.(itemId(item));
      }
    }
  };

  const activeId = optId(activeIndex);
  useEffect(() => keepActiveInView(list.current, activeId), [activeId]);

  usePopupFocus(sheet, () => input.current);

  const prompt = only ? "Type a file name" : sessions ? "Type a command, or a session number" : "Type a command";
  let index = -1;
  const fileList = () =>
    fileGroups.map((g) => (
      <li key={g.title} role="presentation" className="palette__group-item">
        <span className="palette__group" role="presentation">
          {g.title}
        </span>
        <ul role="group" aria-label={g.title} className="palette__sub">
          {items.filter((i) => i.type === "file" && i.group === g.title).map((item) => option(item, ++index))}
        </ul>
      </li>
    ));
  return (
    <div className="palette" onMouseDown={(e) => pressInPalette(e, onClose)}>
      <div className="palette__backdrop" />
      <div ref={sheet} className="palette__sheet" role="dialog" aria-modal="true" aria-label="Command palette" onKeyDown={onKey}>
        <input
          ref={input}
          className="palette__input"
          role="combobox"
          aria-expanded={items.length > 0}
          aria-controls={items.length > 0 ? listId : undefined}
          aria-activedescendant={items.length > 0 ? optId(activeIndex) : undefined}
          aria-label={prompt}
          placeholder={prompt}
          value={query}
          onChange={(e) => onQuery?.(e.target.value)}
        />
        {items.length === 0 ? (
          <p className="palette__empty" role="status">
            {empty}
          </p>
        ) : (
          <ul ref={list} className="palette__list" id={listId} role="listbox" aria-label={only ? "Files" : sessions ? "Sessions and commands" : "Commands"}>
            {only ? (
              fileList()
            ) : filtering ? (
              <>
                {items.filter((i) => i.type !== "file").map((item) => option(item, ++index))}
                {fileList()}
              </>
            ) : (
              <>
                {sessionHits.length > 0 && (
                  <li role="presentation" className="palette__group-item">
                    <span className="palette__group" role="presentation">
                      Open sessions
                    </span>
                    <ul role="group" aria-label="Open sessions" className="palette__sub">
                      {items.filter((i) => i.type === "session").map((item) => option(item, ++index))}
                    </ul>
                  </li>
                )}
                {GROUPS.map((g) => (
                  <li key={g} role="presentation" className="palette__group-item">
                    <span className="palette__group" role="presentation">
                      {g}
                    </span>
                    <ul role="group" aria-label={g} className="palette__sub">
                      {items.filter((i): i is Extract<Item, { type: "command" }> => i.type === "command" && i.hit.item.group === g).map((item) => option(item, ++index))}
                    </ul>
                  </li>
                ))}
              </>
            )}
          </ul>
        )}
        {files?.truncated && fileGroups.length > 0 && <p className="palette__empty">Showing the first 20 000 files</p>}
        <footer className="palette__footer">
          <span>
            <kbd>{"↑"}</kbd> <kbd>{"↓"}</kbd> choose {"·"} <kbd>Enter</kbd> run {"·"} <kbd>Esc</kbd> close
          </span>
          <span>Rebind in Settings</span>
        </footer>
      </div>
    </div>
  );
}
