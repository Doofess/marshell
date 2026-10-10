import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
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
};

const COUNTS: PaneCount[] = [1, 2, 3, 4];

/** The popover behind the bar's layout button: how many terminals are on screen, how they are arranged, and the keys. */
export function LayoutMenu({ count, arrangement, auto, os = "windows" }: LayoutMenuProps) {
  const key = (id: string) => shortcutLabel(commandById(id).binding!, os);
  return (
    <div className="layout-menu" role="dialog" aria-label="Split layout">
      <div className="layout-menu__section">
        <span className="layout-menu__label">Terminals on screen</span>
        <div className="layout-menu__counts" role="radiogroup" aria-label="Terminals on screen">
          {COUNTS.map((n) => (
            <button key={n} type="button" role="radio" className="layout-menu__count" aria-checked={n === count} aria-label={`${n} ${n === 1 ? "pane" : "panes"}`}>
              {n}
            </button>
          ))}
        </div>
        <span className="layout-menu__hint">Any number can run. This is how many you see at once.</span>
      </div>

      {count > 1 && (
        <div className="layout-menu__section">
          <span className="layout-menu__label">Arrangement</span>
          <div className="layout-menu__options" role="radiogroup" aria-label="Arrangement">
            {ARRANGEMENTS[count].map((a) => (
              <button key={a} type="button" role="radio" className="layout-menu__option" aria-checked={a === arrangement} aria-label={ARRANGEMENT_LABEL[a]}>
                <LayoutIcon count={count} arrangement={a} />
                <span>{ARRANGEMENT_LABEL[a]}</span>
              </button>
            ))}
          </div>
          <button type="button" role="checkbox" className="layout-menu__auto" aria-checked={auto}>
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
          <dt>Next pane</dt>
          <dd>
            <kbd>{key("next-pane")}</kbd>
          </dd>
        </div>
      </dl>
    </div>
  );
}
