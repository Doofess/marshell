import type { SessionInfo } from "../../generated/SessionInfo";
import type { SessionMenuFacts } from "../sidebar/sessionMenu";
import type { RowModel } from "../sidebar/types";
import { roomForPane } from "../window/viewState";

/** What the window knows about a session, as the row menu's item list needs it. `shown` panes are on screen; `fit` is the most the window has room for. */
export function menuFacts(
  id: string,
  ctx: { infos: SessionInfo[]; rows: RowModel[]; panes: (string | null)[]; shown: number; fit: number },
): SessionMenuFacts {
  const onScreen = ctx.panes.slice(0, ctx.shown);
  const info = ctx.infos.find((i) => i.tab_id === id);
  return {
    resumable: info !== undefined && info.state.kind !== "running",
    task: ctx.rows.find((r) => r.id === id)?.task === true,
    inShownPane: onScreen.includes(id),
    paneAvailable: onScreen.includes(null) || roomForPane(ctx.panes.length, ctx.fit),
  };
}
