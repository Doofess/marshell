import { useEffect, useRef, type KeyboardEvent } from "react";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { usePopupFocus } from "../../lib/usePopupFocus";
import { ARRANGEMENTS, planFor, type Arrangement, type PaneCount } from "./splitLayout";
import "./LayoutMenu.css";

export const ARRANGEMENT_LABEL: Record<Arrangement, string> = {
  single: "Single",
  columns: "Side by side",
  rows: "Stacked",
  grid: "Grid",
  "main-left": "One main, two beside",
  "main-top": "One main on top",
};

const W = 24;
const H = 18;
const GAP = 1.5;

/** The arrangement drawn as one rounded cell per pane, in the colour of the text around it. */
export function LayoutIcon({ count, arrangement, size = 20 }: { count: PaneCount; arrangement: Arrangement; size?: number }) {
  const plan = planFor(count, arrangement);
  return (
    <svg className="layout-icon" width={size} height={(size * H) / W} viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
      {plan.cells.map((c, i) => {
        const x0 = ((c.colStart - 1) / 2 / plan.colTracks) * W;
        const x1 = (c.colEnd / 2 / plan.colTracks) * W;
        const y0 = ((c.rowStart - 1) / 2 / plan.rowTracks) * H;
        const y1 = (c.rowEnd / 2 / plan.rowTracks) * H;
        return <rect key={i} x={x0 + GAP / 2} y={y0 + GAP / 2} width={x1 - x0 - GAP} height={y1 - y0 - GAP} rx="2" fill="currentColor" />;
      })}
    </svg>
  );
}

export type LayoutMenuProps = {
  count: PaneCount;
  arrangement: Arrangement;
  /** True while the app picks the arrangement from the window. */
  auto: boolean;
  os?: Os;
  /** How many the user asked for, when the window is too small to show them all. */
  requested?: number;
  /** The most the window can usefully hold; counts above it are off. */
  max?: PaneCount;
  /** The user picked a count. Left out (stories), the radios do nothing. */
  onCount?: (n: PaneCount) => void;
  /** The user picked an arrangement, or `null` for automatic. */
  onArrange?: (a: Arrangement | null) => void;
  /** Move the focus into the menu (to the checked count) when it appears. */
  focusOnOpen?: boolean;
};

const COUNTS: PaneCount[] = [1, 2, 3, 4];

/**
 * Where an arrow key, Home or End takes the focus inside a radiogroup: the index of the next enabled radio (wrapping),
 * or null for any other key. Disabled radios are skipped.
 */
export function radioTarget(key: string, from: number, enabled: readonly boolean[]): number | null {
  const n = enabled.length;
  const orNull = (i: number) => (i >= 0 ? i : null);
  if (key === "Home") return orNull(enabled.indexOf(true));
  if (key === "End") return orNull(enabled.lastIndexOf(true));
  const step = key === "ArrowRight" || key === "ArrowDown" ? 1 : key === "ArrowLeft" || key === "ArrowUp" ? -1 : 0;
  if (step === 0) return null;
  for (let i = 1; i <= n; i++) {
    const to = (((from + step * i) % n) + n) % n;
    if (enabled[to]) return to;
  }
  return null;
}

/**
 * What an arrow key, Home or End does in a radiogroup: where the focus goes, and whether that radio is chosen too. Only the
 * arrangements choose as the focus moves; in the counts a choice drops panes, so it waits for Space or Enter.
 */
export function radioMove(key: string, from: number, enabled: readonly boolean[], chooses: boolean): { focus: number; choose: boolean } | null {
  const focus = radioTarget(key, from, enabled);
  return focus === null ? null : { focus, choose: chooses && focus !== from };
}

/** Arrow keys inside a radiogroup move the focus to the next radio; `choose` (when given) also chooses it, like a native one. */
function radioKeys(e: KeyboardEvent<HTMLElement>, enabled: readonly boolean[], choose?: (index: number) => void) {
  if (e.ctrlKey || e.altKey || e.metaKey) return;
  const radios = Array.from(e.currentTarget.parentElement?.querySelectorAll<HTMLElement>('[role="radio"]') ?? []);
  const from = radios.indexOf(e.currentTarget);
  const move = from < 0 ? null : radioMove(e.key, from, enabled, choose !== undefined);
  if (move === null) return;
  e.preventDefault();
  if (move.choose) choose?.(move.focus);
  radios[move.focus]?.focus();
}

/** The popover behind the bar's layout button: how many terminals are on screen, how they are arranged, and the keys. */
export function LayoutMenu({ count, arrangement, auto, os = "windows", requested = count, max = 4, onCount, onArrange, focusOnOpen = false }: LayoutMenuProps) {
  const key = (id: string) => shortcutLabel(commandById(id).binding!, os);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focusOnOpen) root.current?.querySelector<HTMLElement>('[role="radio"][aria-checked="true"]')?.focus();
  }, [focusOnOpen]);
  usePopupFocus(root, () => root.current?.querySelector<HTMLElement>('[role="radio"][aria-checked="true"]') ?? null, { enabled: focusOnOpen });
  const countsOn = COUNTS.map((n) => n <= max);
  const options = ARRANGEMENTS[count];
  return (
    <div ref={root} className="layout-menu" role="dialog" aria-label="Split layout">
      <div className="layout-menu__section">
        <span className="layout-menu__label">Terminals on screen</span>
        <div className="layout-menu__counts" role="radiogroup" aria-label="Terminals on screen">
          {COUNTS.map((n) => (
            <button
              key={n}
              type="button"
              role="radio"
              className="layout-menu__count"
              aria-checked={n === requested}
              aria-disabled={n > max ? true : undefined}
              tabIndex={n === requested ? 0 : -1}
              aria-label={`${n} ${n === 1 ? "pane" : "panes"}`}
              title={n > max ? `This window is too small for ${n} useful terminals` : undefined}
              onClick={() => n <= max && n !== requested && onCount?.(n)}
              onKeyDown={(e) => radioKeys(e, countsOn)}
            >
              {n}
            </button>
          ))}
        </div>
        <span className="layout-menu__hint">Any number can run. This is how many you see at once.</span>
        {max < 4 && <span className="layout-menu__note">This window fits {max}. To see more, make it larger.</span>}
        {requested > count && (
          <span className="layout-menu__note">
            Showing {count} of {requested}. The other {requested - count} keep running.
          </span>
        )}
      </div>

      {count > 1 && (
        <div className="layout-menu__section">
          <span className="layout-menu__label">Arrangement</span>
          <div className="layout-menu__options" role="radiogroup" aria-label="Arrangement">
            {options.map((a) => (
              <button
                key={a}
                type="button"
                role="radio"
                className="layout-menu__option"
                aria-checked={a === arrangement}
                tabIndex={a === arrangement ? 0 : -1}
                aria-label={ARRANGEMENT_LABEL[a]}
                onClick={() => onArrange?.(a)}
                onKeyDown={(e) =>
                  radioKeys(
                    e,
                    options.map(() => true),
                    (i) => onArrange?.(options[i]!),
                  )
                }
              >
                <LayoutIcon count={count} arrangement={a} />
                <span>{ARRANGEMENT_LABEL[a]}</span>
              </button>
            ))}
          </div>
          <button type="button" role="checkbox" className="layout-menu__auto" aria-checked={auto} onClick={() => !auto && onArrange?.(null)}>
            <span className="layout-menu__box" aria-hidden="true" />
            <span>
              <span className="layout-menu__auto-title">Automatic</span>
              <span className="layout-menu__hint">picks the layout that suits the window</span>
            </span>
          </button>
        </div>
      )}

      <dl className="layout-menu__keys">
        <div>
          <dt>Add a pane</dt>
          <dd>
            <kbd>{key("split")}</kbd>
          </dd>
        </div>
        <div>
          <dt>Zoom the active pane</dt>
          <dd>
            <kbd>{key("zoom-pane")}</kbd>
          </dd>
        </div>
        <div>
          <dt>Select the next terminal</dt>
          <dd>
            <kbd>{key("nav-next-pane")}</kbd>
          </dd>
        </div>
      </dl>
    </div>
  );
}
