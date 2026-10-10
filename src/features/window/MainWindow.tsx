import type { CSSProperties } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { ApproveCard } from "../approval/ApproveCard";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { pending } from "../lane/laneLayout";
import type { RowModel } from "../sidebar/types";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { DRAWER, drawerPlacement, sidebarMode, sidebarWidth, type SidebarPref } from "./layout";
import { Rail } from "./Rail";
import { SCENARIO_FOOTER, SCENARIO_PORTS, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { SessionHeader } from "./SessionHeader";
import { Sidebar } from "./Sidebar";
import "./MainWindow.css";

/** The drawer shows the request's full payload; the 40-line command is the long one. */
const DRAWER_DETAIL = APPROVALS.longCommand.detail;

export type WindowLayout = "default" | "split" | "focus" | "rail";
export type MainWindowProps = {
  width: number;
  height: number;
  layout?: WindowLayout;
  sidebarPref?: SidebarPref;
  rows?: RowModel[];
  requests?: ApprovalRequest[];
  selectedId?: string;
  os?: Os;
  terminalSetting?: TerminalThemeSetting;
  headerPeek?: boolean;
  drawer?: boolean;
};

const icon = (d: string) => (
  <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1">
    <path d={d} />
  </svg>
);

function Pane({ row, active, setting }: { row: RowModel; active: boolean; setting: TerminalThemeSetting }) {
  return (
    <section className="pane" data-active={active} aria-label={`Terminal: ${row.name}`}>
      <header className="pane__header">
        <AgentMark agent={row.agent} size={12} />
        <span className="pane__name" dir="auto">
          {row.name}
        </span>
        <span className="pane__place" dir="auto">
          {row.project}
        </span>
      </header>
      <ScriptedTerminal setting={setting} label={`Terminal for ${row.name}`} />
    </section>
  );
}

/** The main window composite (docs/PLAN.md deliverable 5): a Windows-style frame around the sidebar or rail, the session header and the terminal. */
export function MainWindow({
  width,
  height,
  layout = "default",
  sidebarPref,
  rows = SCENARIO_ROWS,
  requests = SCENARIO_REQUESTS,
  selectedId = "working",
  os = "windows",
  terminalSetting = "follow-app",
  headerPeek = false,
  drawer = false,
}: MainWindowProps) {
  const pref: SidebarPref = sidebarPref ?? (layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded");
  const mode = sidebarMode(width, pref);
  const sideW = sidebarWidth(mode);
  const selected = rows.find((r) => r.id === selectedId) ?? rows[0]!;
  const second = rows.find((r) => r.id !== selected.id) ?? selected;
  const waiting = pending(requests).length;
  const showHeader = mode !== "focus" || headerPeek;
  const placement = drawerPlacement(width, mode, DRAWER.default);
  const pillKeys = shortcutLabel(commandById("next-waiting").binding!, os);

  return (
    <div className="window" style={{ inlineSize: width, blockSize: height, "--side-w": `${sideW}px` } as CSSProperties} data-mode={mode} data-layout={layout}>
      <header className="window__bar">
        <div className="window__bar-side">
          <button type="button" className="window__control window__control--menu" aria-label="App menu">
            {icon("M1 2.5h8M1 5h8M1 7.5h8")}
          </button>
          <button type="button" className="window__control window__control--menu" aria-label="Command palette">
            {icon("M1 5h8M5 1v8")}
          </button>
        </div>
        <div className="window__bar-main">{showHeader && <SessionHeader row={selected} ports={SCENARIO_PORTS} />}</div>
        <div className="window__controls">
          <button type="button" className="window__control" aria-label="Minimize">
            {icon("M0 5h10")}
          </button>
          <button type="button" className="window__control" aria-label="Maximize">
            {icon("M0.5 0.5h9v9h-9z")}
          </button>
          <button type="button" className="window__control window__control--close" aria-label="Close">
            {icon("M0 0l10 10M10 0L0 10")}
          </button>
        </div>
      </header>

      <div className="window__body">
        {mode === "expanded" && <Sidebar rows={rows} requests={requests} height={height - 40} width={sideW} selectedId={selected.id} footer={SCENARIO_FOOTER} />}
        {mode === "rail" && <Rail rows={rows} needsYou={waiting} selectedId={selected.id} />}

        <main className="window__main" data-placement={drawer ? placement : undefined}>
          {layout === "split" ? (
            <div className="split">
              <Pane row={selected} active setting={terminalSetting} />
              <div className="split__handle" role="separator" aria-orientation="vertical" aria-label="Resize panes" aria-valuenow={50} aria-valuemin={20} aria-valuemax={80} tabIndex={0} />
              <Pane row={second} active={false} setting={terminalSetting} />
            </div>
          ) : (
            <ScriptedTerminal setting={terminalSetting} label={`Terminal for ${selected.name}`} />
          )}

          {mode === "focus" && waiting > 0 && (
            <button type="button" className="focus-pill" aria-label={`${waiting} need you ${pillKeys}`}>
              <span aria-hidden="true">
                <StatusGlyph kind="needs-permission" />
              </span>
              <span>{waiting} need you</span>{" "}
              <kbd className="focus-pill__kbd">{pillKeys}</kbd>
            </button>
          )}

          {drawer && requests[0] && (
            <aside className="drawer" data-placement={placement} style={{ inlineSize: DRAWER.default }} aria-label="Request details">
              <ApproveCard request={{ ...requests[0], detail: DRAWER_DETAIL }} expanded />
            </aside>
          )}
        </main>
      </div>
    </div>
  );
}
