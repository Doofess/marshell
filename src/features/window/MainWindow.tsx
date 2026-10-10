import type { CSSProperties } from "react";
import { Logo } from "../../components/Logo/Logo";
import { Wordmark } from "../../components/Logo/Wordmark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { ApproveCard } from "../approval/ApproveCard";
import { Composer } from "../composer/Composer";
import { StatusBar } from "../composer/StatusBar";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { pending } from "../lane/laneLayout";
import type { RowModel } from "../sidebar/types";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import { SHELL_SIZE, shellSession } from "../terminal/scriptedShell";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { COMPOSER, DRAWER, STATUS_BAR_HEIGHT, drawerPlacement, sidebarMode, sidebarWidth, type SidebarPref } from "./layout";
import { LayoutIcon, LayoutMenu } from "./LayoutMenu";
import { SearchIcon } from "./SearchIcon";
import { isTool } from "../../components/AgentMark/kinds";
import { hasPromptLine } from "../sidebar/promptLine";
import { Rail } from "./Rail";
import { SCENARIO_FOOTER, SCENARIO_PLAN, SCENARIO_PORTS, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { SessionHeader } from "./SessionHeader";
import { Sidebar } from "./Sidebar";
import { shellNameFor } from "./ShellPanel";
import { ARRANGEMENTS, chooseArrangement, maxPanes, type Arrangement, type PaneCount, type Ratios } from "./splitLayout";
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
  /** The prompt lines (one under a single terminal, one inside every pane in split view); on by default, never in focus mode. */
  composer?: boolean;
  /** What has been typed in the single window's prompt line. */
  composerValue?: string;
  /** What has been typed in each pane's own prompt line in split view, by session id. */
  composerValues?: Record<string, string>;
  /** The 24 px status bar at the foot of the window (the focused session's strip); on by default, never in focus mode. */
  statusBar?: boolean;
  /** The plain shell panel under the session list; on by default in the expanded sidebar. */
  shell?: boolean;
  /** The user hid the shell panel: header only. */
  shellCollapsed?: boolean;
};

const icon = (d: string) => (
  <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1">
    <path d={d} />
  </svg>
);

/**
 * The main window composite (docs/PLAN.md deliverable 5): a Windows-style frame around the sidebar or rail, the session header and the terminal.
 *
 * Prompt lines and focus. Every session on screen has its OWN prompt line: one full line under a single terminal, and in split view
 * one compact line docked inside each pane, so a line of text always goes to the session of the pane it was typed in.
 * Focus is still one thing: clicking a pane (or Ctrl+Shift+[ and ] to move between panes) focuses it, and the header's session,
 * the vendor accent and the status bar at the foot of the window all follow the focused pane. Ctrl+Shift+M puts the cursor in the
 * focused pane's prompt line and never changes which pane is focused. The status bar is the one thing that is not per pane.
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
  composerValues,
  statusBar = true,
  shell = true,
  shellCollapsed = false,
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
  // A manual CLI is a plain terminal: no full prompt line under a single one, and no compact line in its pane.
  const showFull = showComposer && !split && hasPromptLine(selected);
  const showStatus = statusBar && mode !== "focus";
  const pushed = drawer && placement === "push" ? DRAWER.default : 0;
  // The status bar always takes its 24 px. A single terminal also gives up the full prompt line; in split view each pane's own
  // prompt line is counted inside the pane math (splitLayout), not here.
  const requested = Math.min(splitCount, rows.length);
  const candidates = [selected, ...rows.filter((r) => r.id !== selected.id)].slice(0, requested);
  const lineH = split && showComposer && candidates.some(hasPromptLine) ? COMPOSER.compact : 0;
  const space = { w: width - sideW - pushed, h: height - BAR - (showStatus ? STATUS_BAR_HEIGHT : 0) - (showFull ? COMPOSER.full : 0) };
  // Four only where four are usable: a window too small for the count shows fewer, and the rest keep running.
  const visible = split ? Math.min(requested, maxPanes(space, lineH)) : 1;
  const shown: (RowModel | null)[] = [selected, ...rows.filter((r) => r.id !== selected.id)].slice(0, visible);
  if (emptySlot && shown.length > 1) shown[shown.length - 1] = null;
  const choices = rows.filter((r) => !shown.some((s) => s?.id === r.id));
  // Zoom makes its pane the focused one, so the header, the accent bar and the prompt bar can never disagree about which pane is in play.
  const zoomIdx = split && zoomedPane !== undefined && shown[zoomedPane] != null ? zoomedPane : undefined;
  const clamped = Math.min(Math.max(0, Math.floor(Number.isFinite(activePane) ? activePane : 0)), Math.max(0, shown.length - 1));
  const focusedIndex = zoomIdx ?? clamped;
  const focused = shown[focusedIndex] ?? null;
  const active = split ? (focused ?? selected) : selected;
  const paneCount = (split ? shown.length : 1) as PaneCount;
  // An arrangement picked for another number of panes (the window shrank, or fewer sessions exist) gives way to automatic.
  const chosen = arrangement && ARRANGEMENTS[zoomIdx !== undefined ? 1 : paneCount]?.includes(arrangement) ? arrangement : undefined;
  const arranged = split ? (zoomIdx !== undefined ? "single" : (chosen ?? chooseArrangement(paneCount, space, {}, lineH))) : "single";
  const menuCount = (split ? (zoomIdx !== undefined ? 1 : paneCount) : 1) as PaneCount;
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
                <LayoutMenu count={menuCount} requested={split ? requested : 1} max={maxPanes(space, lineH)} arrangement={arranged} auto={chosen === undefined} os={os} />
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
        {mode === "expanded" && <Sidebar rows={rows} requests={requests} height={height - BAR} width={sideW} selectedId={selected.id} footer={SCENARIO_FOOTER} shell={shell ? { cwd: active.project, shellName: shellNameFor(os), collapsed: shellCollapsed, os } : undefined} />}
        {mode === "rail" && <Rail rows={rows} needsYou={waiting} selectedId={selected.id} os={os} />}

        <div className="window__center">
        <main className="window__main" data-placement={drawer ? placement : undefined}>
          {split ? (
            <SplitView panes={shown} size={{ w: space.w, h: space.h }} activeIndex={focusedIndex} hidden={hiddenCount} arrangement={chosen} ratios={ratios} zoomed={zoomIdx} choices={choices} composerValues={composerValues} composers={showComposer} setting={terminalSetting} os={os} />
          ) : (
            <ScriptedTerminal setting={terminalSetting} label={`Terminal for ${selected.name}`} {...(isTool(selected.agent) ? { script: shellSession({ cwd: selected.project }), size: SHELL_SIZE } : {})} />
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
        {showFull && (
          <div className="window__composer" style={{ blockSize: COMPOSER.full }}>
            <Composer row={selected} variant="full" value={composerValue} os={os} />
          </div>
        )}
        {showStatus && (
          <div className="window__status" style={{ blockSize: STATUS_BAR_HEIGHT }}>
            <StatusBar row={split ? focused : selected} plan={isTool((split ? focused : selected)?.agent ?? "") ? undefined : SCENARIO_PLAN} ports={SCENARIO_PORTS} os={os} />
          </div>
        )}
        </div>
      </div>
    </div>
  );
}
