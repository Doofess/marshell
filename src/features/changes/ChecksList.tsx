import { type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, useRef, useState } from "react";
import { ContextMenu, type MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { CiCheck } from "../../generated/CiCheck";
import { githubCopy as c } from "./githubCopy";
import type { ChecksView } from "./pullModel";

export type CheckAction = "open" | "fix" | "copy_link";

/** "48 s", "2 min", "1 min 12 s", "1 h 5 min". Null for a span that isn't known or runs backwards. */
export function durationText(ms: number): string | null {
  if (!Number.isFinite(ms) || ms < 0) return null;
  const total = Math.round(ms / 1000);
  if (total < 60) return `${total} s`;
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  if (minutes < 60) return seconds === 0 ? `${minutes} min` : `${minutes} min ${seconds} s`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours} h` : `${hours} h ${rest} min`;
}

/** How long a check ran: start to end when done, start to `now` while it runs; null when the times aren't both known. */
export function checkDuration(check: CiCheck, now: number): string | null {
  if (check.started_ms === null) return null;
  const end = check.state === "running" ? now : check.completed_ms;
  return end === null ? null : durationText(end - check.started_ms);
}

/** The sentence a screen reader reads for a row: "Failed: clippy, ci, 48 s". */
export function checkSentence(check: CiCheck, now: number): string {
  const parts = [check.name];
  if (check.workflow !== null && check.workflow !== "") parts.push(check.workflow);
  const took = checkDuration(check, now);
  if (took !== null) parts.push(took);
  return `${c.checkStates[check.state]}: ${parts.join(", ")}`;
}

export type RovingResult = { focus: number } | null;

/** Where Up, Down, Home and End take the one tab stop of a list of `count` rows; null for any other key. No wrap. */
export function rovingKey(count: number, focus: number, key: string): RovingResult {
  if (count === 0) return null;
  const last = count - 1;
  switch (key) {
    case "ArrowUp":
      return { focus: Math.max(0, focus - 1) };
    case "ArrowDown":
      return { focus: Math.min(last, focus + 1) };
    case "Home":
      return { focus: 0 };
    case "End":
      return { focus: last };
    default:
      return null;
  }
}

/** A row's menu: open, ask the agent to fix (a failed or cancelled check only), copy the link. */
export function checkMenuItems(check: CiCheck, agent: string | null): MenuItem[] {
  const noAddress = check.url === null ? c.noCheckAddress : undefined;
  const items: MenuItem[] = [{ id: "open", label: c.openOnGithub, ...(noAddress ? { disabled: noAddress } : {}) }];
  if (check.state === "failed" || check.state === "cancelled") items.push({ id: "fix", label: agent === null ? c.copyFix : c.askFix(agent) });
  items.push({ id: "copy_link", label: c.copyLink, ...(noAddress ? { disabled: noAddress } : {}) });
  return items;
}

const GLYPHS: Record<CiCheck["state"], string> = { failed: "✕", cancelled: "⊘", running: "●", waiting: "○", passed: "✓", neutral: "–", skipped: "↷" };

type Entry = { kind: "check"; check: CiCheck; key: string } | { kind: "fold" };

export type ChecksListProps = {
  view: ChecksView;
  agent: string | null;
  /** Epoch ms, for the time a running check has run. */
  now: number;
  /** The list's own id, which the disclosure's `aria-controls` names. */
  id: string;
  onCheck(action: CheckAction, check: CiCheck): void;
  onEscape(): void;
};

/** The checks of a pull request: one tab stop with a roving focus; the folded (passed, neutral, skipped) ones sit behind a row. */
export function ChecksList({ view, agent, now, id, onCheck, onEscape }: ChecksListProps) {
  const [folded, setFolded] = useState(false);
  const [focus, setFocus] = useState(0);
  const [menu, setMenu] = useState<{ check: CiCheck; anchor: { x: number; y: number }; opener: HTMLElement | null } | null>(null);
  const list = useRef<HTMLUListElement>(null);

  const entries: Entry[] = view.listed.map((check, i) => ({ kind: "check", check, key: `l${i}` }));
  if (view.folded.length > 0) entries.push({ kind: "fold" });
  if (folded) view.folded.forEach((check, i) => entries.push({ kind: "check", check, key: `f${i}` }));
  const at = Math.min(focus, Math.max(0, entries.length - 1));

  const move = (i: number) => {
    setFocus(i);
    list.current?.querySelector<HTMLElement>(`[data-check-index="${i}"]`)?.focus();
  };
  const toggle = () => setFolded((f) => !f);

  const onKey = (e: ReactKeyboardEvent<HTMLElement>, index: number) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const entry = entries[index]!;
    if (e.key === "Escape") {
      e.preventDefault();
      return onEscape();
    }
    const to = rovingKey(entries.length, index, e.key);
    if (to !== null) {
      e.preventDefault();
      return move(to.focus);
    }
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (entry.kind === "fold") toggle();
      else if (e.key === "Enter") onCheck("open", entry.check);
      return;
    }
    if (entry.kind === "check" && (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey))) {
      e.preventDefault();
      const r = e.currentTarget.getBoundingClientRect();
      setMenu({ check: entry.check, anchor: { x: r.left + 24, y: r.bottom }, opener: e.currentTarget });
    }
  };

  const closeMenu = (refocus: boolean) => {
    const m = menu;
    setMenu(null);
    if (refocus) m?.opener?.focus();
  };
  const run = (choice: string) => {
    const m = menu;
    closeMenu(true);
    if (m !== null) onCheck(choice as CheckAction, m.check);
  };

  return (
    <div className="pull-bar__checks" id={id}>
      <ul ref={list} className="pull-bar__list" aria-label={c.checksList}>
        {entries.map((entry, i) => {
          const common = { "data-check-index": i, tabIndex: i === at ? 0 : -1, onKeyDown: (e: ReactKeyboardEvent<HTMLElement>) => onKey(e, i), onFocus: () => setFocus(i) };
          if (entry.kind === "fold") {
            return (
              <li key="fold" className="pull-bar__item">
                <div {...common} role="button" aria-expanded={folded} className="pull-bar__check pull-bar__check--fold" onClick={toggle}>
                  <span className="pull-bar__glyph" aria-hidden="true">
                    {folded ? "▾" : "▸"}
                  </span>
                  <span className="pull-bar__check-name">{view.foldedLabel}</span>
                </div>
              </li>
            );
          }
          const { check } = entry;
          const took = checkDuration(check, now);
          return (
            <li key={entry.key} className="pull-bar__item">
              <div
                {...common}
                role="button"
                aria-label={checkSentence(check, now)}
                data-state={check.state}
                className="pull-bar__check"
                onClick={() => onCheck("open", check)}
                onContextMenu={(e: ReactMouseEvent<HTMLElement>) => {
                  e.preventDefault();
                  setFocus(i);
                  setMenu({ check, anchor: { x: e.clientX, y: e.clientY }, opener: e.currentTarget });
                }}
              >
                <span className="pull-bar__glyph" aria-hidden="true">
                  {GLYPHS[check.state]}
                </span>
                <span className="pull-bar__check-state" aria-hidden="true">
                  {c.checkStates[check.state]}
                </span>
                <span className="pull-bar__check-name" aria-hidden="true">
                  {check.name}
                </span>
                {check.workflow !== null && check.workflow !== "" && (
                  <span className="pull-bar__check-extra" aria-hidden="true">
                    {check.workflow}
                  </span>
                )}
                {took !== null && (
                  <span className="pull-bar__check-extra" aria-hidden="true">
                    {took}
                  </span>
                )}
              </div>
              {check.state === "failed" && (
                <button type="button" tabIndex={-1} className="pull-bar__button pull-bar__button--small" onClick={() => onCheck("fix", check)}>
                  {agent === null ? c.copyFix : c.askFix(agent)}
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {view.more > 0 && <p className="pull-bar__more">{c.andMore(view.more)}</p>}
      {menu && <ContextMenu label={c.checkMenu(menu.check.name)} items={checkMenuItems(menu.check, agent)} anchor={menu.anchor} onRun={run} onClose={({ refocus }) => closeMenu(refocus)} />}
    </div>
  );
}
