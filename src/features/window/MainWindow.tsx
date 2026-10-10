import type { CSSProperties } from "react";
import { Logo } from "../../components/Logo/Logo";
import { Wordmark } from "../../components/Logo/Wordmark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { ApproveCard } from "../approval/ApproveCard";
import { Composer } from "../composer/Composer";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { pending } from "../lane/laneLayout";
import type { RowModel } from "../sidebar/types";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { COMPOSER_HEIGHT, DRAWER, drawerPlacement, sidebarMode, sidebarWidth, type SidebarPref } from "./layout";
import { LayoutIcon, LayoutMenu } from "./LayoutMenu";
import { SearchIcon } from "./SearchIcon";
import { Rail } from "./Rail";
import { SCENARIO_FOOTER, SCENARIO_PORTS, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { SessionHeader } from "./SessionHeader";
import { Sidebar } from "./Sidebar";
import { chooseArrangement, maxPanes, type Arrangement, type PaneCount, type Ratios } from "./splitLayout";
import { SplitView } from "./SplitView";
import "./MainWindow.css";

/** The drawer shows the request's full payload; the 40-line command is the long one. */
const DRAWER_DETAIL = APPROVALS.longCommand.detail;
const BAR = 40;

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
  /** Split view: how many terminals are on screen (2 to 4). */
  splitCount?: PaneCount;
  /** Leave out to let the app pick from the space. */
  arrangement?: Arrangement;
  ratios?: Ratios;
  activePane?: number;
  /** Index of the pane that fills the view. */
  zoomedPane?: number;
  /** Make the last pane an empty slot that offers the sessions not on screen. */
  emptySlot?: boolean;
  /** Show the layout menu open. */
  layoutMenu?: boolean;
  /** The prompt bar under the terminals; on by default, never in focus mode. */
  composer?: boolean;
  /** What has been typed in it. */
  composerValue?: string;
};

const icon = (d: string) => (
  <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1">
    <path d={d} />
  </svg>
);

/**
 * The main window composite (docs/PLAN.md deliverable 5): a Windows-style frame around the sidebar or rail, the session header and the terminal.
 *
 * Focus model for the prompt bar: the bar under the terminals writes to the FOCUSED pane's session. Focusing a pane (a click,
 * or Ctrl+Shift+[ and ] to move between panes) changes the target, the header's session and the vendor accent together;
 * Ctrl+Shift+M focuses the prompt bar and never changes which pane is focused. In a single terminal the target is that session.
 */
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
  splitCount = 2,
  arrangement,
  ratios,
  activePane = 0,
  zoomedPane,
  emptySlot = false,
  layoutMenu = false,
  composer = true,
  composerValue = "",
}: MainWindowProps) {
  const pref: SidebarPref = sidebarPref ?? (layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded");
  const mode = sidebarMode(width, pref);
  const sideW = sidebarWidth(mode);
  const selected = rows.find((r) => r.id === selectedId) ?? rows[0]!;
  const waiting = pending(requests).length;
  const showHeader = mode !== "focus" || headerPeek;
  const placement = drawerPlacement(width, mode, DRAWER.default);
  const pillKeys = shortcutLabel(commandById("next-waiting").binding!, os);

  // Split view: the selected session first, then the others in order, never the same one twice.
  const split = layout === "split";
  const showComposer = composer && mode !== "focus";
  const pushed = drawer && placement === "push" ? DRAWER.default : 0;
  const space = { w: width - sideW - pushed, h: height - BAR - (showComposer ? COMPOSER_HEIGHT : 0) };
  // Four only where four are usable: a window too small for the count shows fewer, and the rest keep running.
  const requested = Math.min(splitCount, rows.length);
  const visible = split ? Math.min(requested, maxPanes(space)) : 1;
  const shown: (RowModel | null)[] = [selected, ...rows.filter((r) => r.id !== selected.id)].slice(0, visible);
  if (emptySlot && shown.length > 1) shown[shown.length - 1] = null;
  const choices = rows.filter((r) => !shown.some((s) => s?.id === r.id));
  const focusedIndex = activePane < shown.length ? activePane : 0;
  const focused = shown[focusedIndex] ?? null;
  const active = split ? (focused ?? selected) : selected;
  const paneCount = (split ? shown.length : 1) as PaneCount;
  const arranged = split ? (zoomedPane !== undefined ? "single" : (arrangement ?? chooseArrangement(paneCount, space))) : "single";
  const menuCount = (split ? (zoomedPane !== undefined ? 1 : paneCount) : 1) as PaneCount;
  const hiddenCount = split ? requested - shown.length : 0;

  return (
    <div className="window" style={{ inlineSize: width, blockSize: height, "--side-w": `${sideW}px` } as CSSProperties} data-mode={mode} data-layout={layout} data-agent={active.agent}>
      <header className="window__bar">
        <div className="window__bar-side">
          <button type="button" className="window__brand" aria-label="Marshell menu" aria-haspopup="menu">
            <Logo size={22} decorative />
            {mode === "expanded" && <Wordmark height={13} decorative />}
          </button>
          {mode === "expanded" && (
            <button type="button" className="window__palette" aria-label="Command palette" title={`Search sessions, projects and commands (${shortcutLabel(commandById("palette").binding!, os)})`}>
              <SearchIcon />
              <span>Command palette</span>
            </button>
          )}
        </div>
        <div className="window__bar-main">{showHeader && <SessionHeader row={active} ports={SCENARIO_PORTS} />}</div>
        <div className="window__bar-end">
          <div className="window__layout">
            <button
              type="button"
              className="window__control window__control--menu"
              aria-label="Split layout"
              aria-haspopup="dialog"
              aria-expanded={layoutMenu}
              title="Split layout: up to four terminals on screen"
            >
              <LayoutIcon count={menuCount} arrangement={arranged} size={18} />
            </button>
            {layoutMenu && (
              <div className="window__popover">
                <LayoutMenu count={menuCount} requested={split ? splitCount : 1} max={maxPanes(space)} arrangement={arranged} auto={arrangement === undefined} os={os} />
              </div>
            )}
          </div>
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
        </div>
      </header>

      <div className="window__body">
        {mode === "expanded" && <Sidebar rows={rows} requests={requests} height={height - BAR} width={sideW} selectedId={selected.id} footer={SCENARIO_FOOTER} />}
        {mode === "rail" && <Rail rows={rows} needsYou={waiting} selectedId={selected.id} os={os} height={height} />}

        <div className="window__center">
        <main className="window__main" data-placement={drawer ? placement : undefined}>
          {split ? (
            <SplitView panes={shown} size={{ w: space.w, h: space.h }} activeIndex={focusedIndex} hidden={hiddenCount} arrangement={arrangement} ratios={ratios} zoomed={zoomedPane} choices={choices} setting={terminalSetting} os={os} />
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
        {showComposer && (
          <Composer row={split ? focused : selected} value={composerValue} os={os} pane={split ? { index: focusedIndex, count: shown.length } : undefined} />
        )}
        </div>
      </div>
    </div>
  );
}
