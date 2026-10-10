import type { KeyboardEvent } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import { COMMANDS, commandById, shortcutLabel, type Command, type Os } from "../../lib/keymap";
import { filterCommands, filterSessions, highlight, parseSessionNumber, sessionDetail, type Hit, type Range, type SessionHit } from "../launcher/filter";
import type { PaletteSession } from "./fixtures";
import "./Palette.css";

export type PaletteView = { query: string; activeIndex: number };
export type { PaletteSession };

const GROUPS: Command["group"][] = ["Sessions", "View", "Tools"];
const FOCUS_SESSION = /^focus-session-\d+$/;

type Item = { type: "session"; hit: SessionHit<PaletteSession> } | { type: "command"; hit: Hit<Command> };
const itemId = (i: Item) => (i.type === "session" ? `session:${i.hit.item.id}` : i.hit.item.id);

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

type OptionProps = { index: number; active: boolean; os: Os; onRun?: (id: string) => void };

function CommandOption({ hit, index, active, os, onRun }: OptionProps & { hit: Hit<Command> }) {
  const c = hit.item;
  return (
    <li id={`palette-opt-${index}`} role="option" aria-selected={active} className="palette__opt" data-active={active || undefined} onClick={() => onRun?.(c.id)}>
      <span className="palette__label" dir="auto">
        <Marked text={c.label} ranges={hit.ranges} />
      </span>
      {c.binding && <kbd className="palette__kbd">{shortcutLabel(c.binding, os)}</kbd>}
    </li>
  );
}

/** One open session: its number first, then its mark, name and where it runs. Sessions 1 to 9 show the Ctrl+digit that goes there. */
function SessionOption({ hit, index, active, os, onRun }: OptionProps & { hit: SessionHit<PaletteSession> }) {
  const s = hit.item;
  const detail = sessionDetail(s);
  const key = s.number <= 9 ? commandById(`focus-session-${s.number}`).binding : undefined;
  return (
    <li
      id={`palette-opt-${index}`}
      role="option"
      aria-selected={active}
      aria-label={`Go to session ${s.number}, ${s.name} ${detail}${key ? ` ${shortcutLabel(key, os)}` : ""}, ${GLYPHS[s.status].label}`}
      className="palette__opt"
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

/**
 * The command palette. Every action in the app is here, with its shortcut shown; shortcuts are rebindable in Settings.
 * With `sessions` it is also where you go to a session: each open one is a row with its number, and typing 6, #6 or
 * "session 6" puts that session first (the nine "Go to session N" commands give way to the rows, which show the same
 * Ctrl+digit). Presentational: the parent owns the query and the active row.
 */
export function Palette({
  view,
  sessions,
  os = "windows",
  onQuery,
  onRun,
  onActive,
  onClose,
}: {
  view: PaletteView;
  sessions?: PaletteSession[];
  os?: Os;
  onQuery?: (q: string) => void;
  /** Receives a command's id, or `session:<id>` for a session row. */
  onRun?: (id: string) => void;
  onActive?: (index: number) => void;
  onClose?: () => void;
}) {
  const { query, activeIndex } = view;
  const filtering = query.trim() !== "";
  const wanted = sessions ? parseSessionNumber(query) : null;

  const sessionHits = sessions ? filterSessions(query, sessions) : [];
  const pool = sessions ? COMMANDS.filter((c) => !FOCUS_SESSION.test(c.id)) : COMMANDS;
  // A number means a session: no command should answer to "4" because its label says "up to 4".
  const commandHits = wanted !== null ? [] : filterCommands(query, pool);
  const items: Item[] = [...sessionHits.map((hit): Item => ({ type: "session", hit })), ...commandHits.map((hit): Item => ({ type: "command", hit }))];

  const option = (item: Item, index: number) =>
    item.type === "session" ? (
      <SessionOption key={itemId(item)} hit={item.hit} index={index} active={index === activeIndex} os={os} onRun={onRun} />
    ) : (
      <CommandOption key={itemId(item)} hit={item.hit} index={index} active={index === activeIndex} os={os} onRun={onRun} />
    );

  const numbers = sessions ? [...sessions].map((s) => s.number).sort((a, b) => a - b) : [];
  const empty =
    wanted !== null
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

  const prompt = sessions ? "Type a command, or a session number" : "Type a command";
  let index = -1;
  return (
    <div className="palette">
      <div className="palette__backdrop" />
      <div className="palette__sheet" role="dialog" aria-modal="true" aria-label="Command palette" onKeyDown={onKey}>
        <input
          className="palette__input"
          role="combobox"
          aria-expanded={items.length > 0}
          aria-controls={items.length > 0 ? "palette-list" : undefined}
          aria-activedescendant={items.length > 0 ? `palette-opt-${activeIndex}` : undefined}
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
          <ul className="palette__list" id="palette-list" role="listbox" aria-label={sessions ? "Sessions and commands" : "Commands"}>
            {filtering ? (
              items.map((item) => option(item, ++index))
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
