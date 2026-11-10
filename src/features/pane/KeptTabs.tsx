import { useRef, type MutableRefObject, type ReactNode } from "react";
import type { PaneTab } from "../../generated/PaneTab";
import { keptTabs } from "./paneState";

/**
 * The panels of the kept tabs: the selected one and the last few shown, in the order of the tabs so a panel's DOM node never moves
 * when the selection does. A panel that isn't selected stays mounted and carries `hidden`. `keptRef` is for tests.
 */
export function KeptTabs({ tabs, activeId, render, keptRef }: { tabs: PaneTab[]; activeId: string; render(tab: PaneTab, active: boolean): ReactNode; keptRef?: MutableRefObject<string[]> }) {
  const own = useRef<string[]>([]);
  const ref = keptRef ?? own;
  ref.current = keptTabs(ref.current, activeId, tabs.map((t) => t.id));
  const kept = new Set(ref.current);
  return (
    <>
      {tabs
        .filter((t) => kept.has(t.id))
        .map((t) => {
          const active = t.id === activeId;
          return (
            <div key={t.id} role="tabpanel" id={`pane-panel-${t.id}`} aria-labelledby={`pane-tab-${t.id}`} className="side-pane__panel" hidden={!active}>
              {render(t, active)}
            </div>
          );
        })}
    </>
  );
}
