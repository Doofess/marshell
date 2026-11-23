import { type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, useId, useState } from "react";
import { ContextMenu, type MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { CiCheck } from "../../generated/CiCheck";
import type { MergeMethod } from "../../generated/MergeMethod";
import { ChecksList, type CheckAction } from "./ChecksList";
import { githubCopy as c } from "./githubCopy";
import type { BarButton, BarMenuItem, BarView } from "./pullModel";
import "./PullBar.css";

export type PullBarProps = {
  view: BarView;
  /** The session's agent, for the checks' "Ask {agent} to fix"; null for a session with no agent. */
  agent: string | null;
  /** Epoch ms, for the time a running check has run. */
  now: number;
  /** The main button, or one of its other methods (the ▾ menu). */
  onMain(button: BarButton, method?: MergeMethod): void;
  onMenu(item: BarMenuItem): void;
  onCheck(action: CheckAction, check: CiCheck): void;
  onEscape(): void;
};

type Open = { which: "method" | "more"; anchor: { x: number; y: number }; opener: HTMLElement };

/** The pull request bar: one or two lines, the notes, the main button with its menus, and the checks behind a disclosure. */
export function PullBar({ view, agent, now, onMain, onMenu, onCheck, onEscape }: PullBarProps) {
  const uid = useId();
  const [checksOpen, setChecksOpen] = useState(false);
  const [open, setOpen] = useState<Open | null>(null);
  const { main } = view;
  const off = main !== null && main.disabled !== null;
  const reasonId = `${uid}-reason`;
  // A sentence that is already one of the notes is drawn once: the note carries the id the button names.
  const reasonIsNote = off && view.notes.includes(main.disabled ?? "");
  const listId = `${uid}-checks`;
  // "Hide" stands in the bar for the lines that ask for a setup; the others have it in the ⋯ menu only when it isn't shown here.
  const menu = view.menu.filter((i) => !(view.hide && i.id === "hide"));
  const hideItem = view.hide ? view.menu.find((i) => i.id === "hide") : undefined;

  const openFrom = (which: Open["which"], e: ReactMouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setOpen({ which, anchor: { x: r.left, y: r.bottom }, opener: e.currentTarget });
  };
  const close = (refocus: boolean) => {
    const o = open;
    setOpen(null);
    if (refocus) o?.opener.focus();
  };
  const methodItems: MenuItem[] = (main?.others ?? []).map((o) => ({ id: o.method, label: o.label }));
  const moreItems: MenuItem[] = menu.map((i) => ({ id: i.id, label: i.label, ...(i.disabled !== null ? { disabled: i.disabled } : {}) }));
  const run = (id: string) => {
    const o = open;
    close(true);
    if (o === null) return;
    if (o.which === "method" && main) onMain(main, id as MergeMethod);
    else {
      const item = menu.find((i) => i.id === id);
      if (item && item.disabled === null) onMenu(item);
    }
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLElement>) => {
    if (e.key === "Escape" && !e.defaultPrevented) {
      e.preventDefault();
      onEscape();
    }
  };

  return (
    <section className="pull-bar" aria-label={c.barLabel} onKeyDown={onKeyDown}>
      <p className="pull-bar__line">{view.line}</p>
      {view.second !== null && <p className="pull-bar__second">{view.second}</p>}
      {view.notes.map((t) => (
        <p key={t} id={reasonIsNote && t === main.disabled ? reasonId : undefined} className="pull-bar__note">
          {t}
        </p>
      ))}
      <div className="pull-bar__actions">
        {view.checks !== null && (
          <button type="button" className="pull-bar__button" aria-expanded={checksOpen} aria-controls={listId} onClick={() => setChecksOpen((v) => !v)}>
            {view.checks.summary}
          </button>
        )}
        {main !== null && (
          <button
            type="button"
            className="pull-bar__button pull-bar__button--primary"
            aria-disabled={off ? "true" : undefined}
            aria-describedby={off ? reasonId : undefined}
            onClick={() => {
              if (!off) onMain(main);
            }}
          >
            {main.label}
          </button>
        )}
        {main !== null && !off && (main.others?.length ?? 0) > 0 && (
          <button type="button" className="pull-bar__button" aria-label={c.methodButton} aria-haspopup="menu" aria-expanded={open?.which === "method"} onClick={(e) => openFrom("method", e)}>
            <span aria-hidden="true">▾</span>
          </button>
        )}
        {hideItem !== undefined && (
          <button type="button" className="pull-bar__button" onClick={() => onMenu(hideItem)}>
            {hideItem.label}
          </button>
        )}
        {view.link !== null && (
          <button type="button" className="pull-bar__button" onClick={() => onMenu({ id: "open_on_github", label: view.link!.label, disabled: null, url: view.link!.url })}>
            {view.link.label}
          </button>
        )}
        {menu.length > 0 && (
          <button type="button" className="pull-bar__button" aria-label={c.moreButton} aria-haspopup="menu" aria-expanded={open?.which === "more"} onClick={(e) => openFrom("more", e)}>
            <span aria-hidden="true">⋯</span>
          </button>
        )}
      </div>
      {off && !reasonIsNote && (
        <p id={reasonId} className="pull-bar__reason">
          {main.disabled}
        </p>
      )}
      {view.checks !== null && checksOpen && <ChecksList view={view.checks} agent={agent} now={now} id={listId} onCheck={onCheck} onEscape={onEscape} />}
      {open !== null && <ContextMenu label={open.which === "method" ? c.methodMenu : c.moreMenu} items={open.which === "method" ? methodItems : moreItems} anchor={open.anchor} onRun={run} onClose={({ refocus }) => close(refocus)} />}
    </section>
  );
}
