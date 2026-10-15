import { AgentMark } from "../../components/AgentMark/AgentMark";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import type { TrayState } from "./trayItems";
import { trayItems } from "./trayItems";
import "./Tray.css";

/**
 * The tray menu as a layout (design level): the OS draws the real one from the same items. Sessions that wait carry their
 * number and mark. On Linux the tray has no click events, so the menu is all there is; macOS and Linux are design only.
 */
export function TrayMenu({ state, caption = false }: { state: TrayState; caption?: boolean }) {
  const items = trayItems(state);
  const mark = (n: number | undefined) => state.sessions.find((s) => s.number === n);
  return (
    <figure className="tray-frame">
      <div className="tray-menu" role="menu" aria-label="Marshell tray menu" data-os={state.os}>
        {items.map((i) => {
          if (i.kind === "separator") return <div key={i.id} className="tray-menu__sep" role="separator" />;
          const s = i.kind === "session" ? mark(i.number) : undefined;
          return (
            <button key={i.id} type="button" role="menuitem" className="tray-menu__item" aria-disabled={i.disabled || undefined} data-agent={s?.agent}>
              {s && i.number !== undefined && (
                <>
                  <SessionNumber n={i.number} />
                  <AgentMark agent={s.agent} size={12} />{" "}
                </>
              )}
              <span className="tray-menu__label" dir="auto">
                {i.label}
              </span>
              {i.shortcut && <kbd className="tray-menu__kbd">{i.shortcut}</kbd>}
            </button>
          );
        })}
      </div>
      {caption && (
        <figcaption className="tray-frame__note">
          {state.os === "linux" && <>On Linux the tray is menu-only: it has no click events. </>}
          {state.os !== "windows" && <>Design only: macOS and Linux are not tested yet.</>}
        </figcaption>
      )}
    </figure>
  );
}
