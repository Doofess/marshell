import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import type { ApprovalRequest } from "../approval/types";
import type { RowModel } from "../sidebar/types";
import { trayItems, trayTooltip, type TrayItem, type TraySession, type TrayState } from "./trayItems";

const WAITING = new Set<GlyphKind>(["needs-permission", "needs-question"]);

/** What the shell is sent: the menu, the tooltip, and how many are waiting (all of them, not only the ones the menu lists). */
export type TrayModel = { items: TrayItem[]; tooltip: string; waiting: number };

/**
 * The needs-you list as the tray shows it, in the lane's order (oldest first), one per session like the count. A row that is
 * waiting gives the status and the phrase; a held request whose row is missing, or no longer reads as waiting, is still a
 * request for permission, so it stays in the menu the count is made of.
 */
export function traySessions(entries: ApprovalRequest[], rows: RowModel[]): TraySession[] {
  const byTab = new Map(rows.map((r) => [r.id, r]));
  const seen = new Set<string>();
  const out: TraySession[] = [];
  for (const e of entries) {
    if (seen.has(e.tab)) continue;
    seen.add(e.tab);
    const row = byTab.get(e.tab);
    const status: GlyphKind = row && WAITING.has(row.status) ? row.status : "needs-permission";
    out.push({
      number: row?.number ?? e.session.number ?? 0,
      name: row?.name ?? e.session.name,
      status,
      phrase: (row && WAITING.has(row.status) && row.phrase.trim()) || (status === "needs-question" ? "Has a question" : "Needs your permission"),
      agent: row?.agent ?? e.session.agent,
    });
  }
  return out;
}

export function trayModel(state: TrayState): TrayModel {
  return { items: trayItems(state), tooltip: trayTooltip(state), waiting: state.sessions.filter((s) => WAITING.has(s.status)).length };
}

/** What a tray menu choice does in the window. Open and Quit are the shell's own and never get here. */
export type TrayAction = { kind: "goto"; number: number } | { kind: "nextWaiting" } | { kind: "overlay"; overlay: "notifications" | "departures" } | { kind: "togglePause" };

/** The tray's `pause` is the notifications pause, not the palette's Pause session. */
export function actionFor(id: string): TrayAction | null {
  const n = /^session-(\d+)$/.exec(id);
  if (n) return { kind: "goto", number: Number(n[1]) };
  switch (id) {
    case "next-waiting":
    case "more":
      return { kind: "nextWaiting" };
    case "notifications":
      return { kind: "overlay", overlay: "notifications" };
    case "departures":
      return { kind: "overlay", overlay: "departures" };
    case "pause":
      return { kind: "togglePause" };
    default:
      return null;
  }
}

/** Keeps the shell's tray in step with `state`, sending the model only when it changes. `null` until the state is known. */
export function useTray(state: TrayState | null): void {
  const json = state ? JSON.stringify(trayModel(state)) : null;
  useEffect(() => {
    if (json === null) return;
    // No Tauri here (tests, a plain browser), or the shell has no tray: the tray is only a courtesy.
    Promise.resolve(invoke("set_tray", { model: JSON.parse(json) })).catch(() => {});
  }, [json]);
}

/** Calls `onAction` for each tray menu choice the shell passes on. */
export function useTrayActions(onAction: (action: TrayAction) => void): void {
  const latest = useRef(onAction);
  latest.current = onAction;
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    listen<{ id: string }>("tray-action", (e) => {
      const action = actionFor(e.payload.id);
      if (action) latest.current(action);
    })
      .then((s) => (gone ? s() : (stop = s)))
      .catch(() => {}); // Outside Tauri (tests, a plain browser) there are no events.
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
}
