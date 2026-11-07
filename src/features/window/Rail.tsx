import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { MenuIcon } from "./MenuIcon";
import { selectsByCtrlSpace } from "../sidebar/rowPick";
import { rowLabel } from "../sidebar/labels";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import type { RowModel } from "../sidebar/types";
import { selectedProps } from "../../components/a11y/SelectedMark";
import "./Rail.css";

/**
 * The 52 px rail: the brand stripe, the vendor mark and a small status glyph per session, with a needs-you badge stack on top.
 * The marks are pictures the button's name already says, so they are hidden from it. A working session shows its vendor
 * mark moving instead of a glyph, as in the full rows. The list scrolls (wheel, touch,
 * keys) with no scrollbar drawn. The Menu button is pinned under the list, outside the Sessions nav. The palette and sidebar buttons live in the top bar, so they stay put when the rail is shown.
 */
export function Rail({
  rows,
  needsYou,
  selectedId,
  visibleIds,
  os = "windows",
  onSelect,
  onOpenMenu,
  menuOpen,
}: {
  onOpenMenu?: (button: HTMLElement) => void;
  menuOpen?: boolean;
  rows: RowModel[];
  needsYou: number;
  selectedId?: string;
  visibleIds?: readonly string[];
  os?: Os;
  onSelect?: (id: string) => void;
}) {
  return (
    <div className="rail">
      <nav className="rail__nav" aria-label="Sessions">
        <div className="rail__list">
          {needsYou > 0 && (
            <div
              className="rail__badge"
              role="img"
              aria-label={`${needsYou} need you`}
            >
              <StatusGlyph kind="needs-permission" label="Needs you" />
              <span className="rail__count">{needsYou}</span>
            </div>
          )}
          {rows.map((r) => (
            <button
              key={r.id}
              type="button"
              {...selectedProps(
                r.id === selectedId &&
                  (!visibleIds || visibleIds.includes(r.id)),
                "current",
                "rail__item",
              )}
              data-visible={visibleIds?.includes(r.id) || undefined}
              data-agent={r.agent}
              data-number={r.number}
              data-nav-row={r.number}
              aria-label={rowLabel(r)}
              onClick={onSelect && (() => onSelect(r.id))}
              onKeyDown={
                onSelect &&
                ((e) => {
                  if (!selectsByCtrlSpace(e)) return;
                  // Held off the button's own Space press, which would also click it.
                  e.preventDefault();
                  onSelect(r.id);
                })
              }
            >
              <span className="rail__stripe" aria-hidden="true" />
              <span className="rail__number">
                <SessionNumber n={r.number} />
              </span>
              <span className="rail__mark" aria-hidden="true">
                <AgentMark
                  agent={r.agent}
                  {...(r.agentName === undefined ? {} : { label: r.agentName })}
                  size={20}
                  working={r.status === "working"}
                />
                {r.status !== "working" && (
                  <StatusGlyph kind={r.status} size={12} />
                )}
              </span>
            </button>
          ))}
        </div>
      </nav>
      {onOpenMenu && (
        <button
          type="button"
          className="rail__menu"
          aria-label="Menu"
          aria-haspopup="menu"
          aria-expanded={menuOpen === true}
          aria-keyshortcuts={shortcutLabel(commandById("menu").binding!, os)}
          title={`Menu (${shortcutLabel(commandById("menu").binding!, os)})`}
          onClick={(e) => onOpenMenu(e.currentTarget)}
        >
          <MenuIcon />
        </button>
      )}
    </div>
  );
}
