import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { type MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { PortMenuRequest } from "../ports/PortChips";
import { PortMenu } from "../ports/PortMenu";
import { setTitle as setWindowTitle } from "../../lib/window/appWindow";
import { Logo } from "../../components/Logo/Logo";
import { Wordmark } from "../../components/Logo/Wordmark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { ApproveCard } from "../approval/ApproveCard";
import { useCoverPage } from "../browser/pageCover";
import { Composer } from "../composer/Composer";
import { StatusBar } from "../composer/StatusBar";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { pendingSessionCount } from "../lane/laneLayout";
import type { RowModel } from "../sidebar/types";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import { SHELL_SIZE, shellSession } from "../terminal/scriptedShell";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { COMPOSER, DRAWER, STATUS_BAR_HEIGHT, SIDEBAR, drawerPlacement, sidebarWidth as widthFor, type SidebarMode, type SidebarPref } from "./layout";
import { SearchIcon } from "./SearchIcon";
import { SidebarIcon } from "./SidebarIcon";
import { isTool } from "../../components/AgentMark/kinds";
import { hasPromptLine } from "../sidebar/promptLine";
import type { Rect } from "../../lib/navigation/types";
import { KeptTerminals } from "./keptTerminals";
import { NavCursor, NO_SELECTION_TEXT } from "./NavCursor";
import { Rail } from "./Rail";
import { SCENARIO_FOOTER, SCENARIO_PLAN,SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import type { Notice } from "./announce";
import { SessionAnnouncer } from "./SessionAnnouncer";
import { SessionHeader } from "./SessionHeader";
import type { RunControl } from "../scripts/confirmScripts";
import { windowTitle } from "./windowTitle";
import { Sidebar, type SidebarFooter } from "./Sidebar";
import { shellNameFor } from "./ShellPanel";
import { LayoutContext, mainRoom, type Layout } from "./paneColumn";
import { PANE, draggedWidth, paneGeometry, paneMotionKey } from "./paneLayout";
import { useLayoutMotion } from "./motion";
import { FitHoldContext } from "../terminal/fitHold";
import { sidebarToggleFor } from "./viewState";
import "./MainWindow.css";

/** The drawer shows the request's full payload; the 40-line command is the long one. */
const DRAWER_DETAIL = APPROVALS.longCommand.detail;
const BAR = 40;
/** How far one arrow key press moves the sidebar's edge. */
const SIDEBAR_STEP = 8;
const clampSidebar = (w: number) => Math.round(Math.min(SIDEBAR.max, Math.max(SIDEBAR.min, w)));
const WIDTH_RANK: Record<SidebarMode, number> = { focus: 0, rail: 1, expanded: 2 };
/** The widest of the sidebar modes given: while the column moves it shows that one, so nothing in it reflows. */
const widestMode = (...modes: Array<SidebarMode | null>): SidebarMode => modes.reduce<SidebarMode>((w, m) => (m !== null && WIDTH_RANK[m] > WIDTH_RANK[w] ? m : w), "focus");

export type WindowLayout = "default" | "focus" | "rail";
/** What makes the sidebar's shell panel live (see `MainWindowProps.shell`). */
export type ShellOptions = { terminal?: ReactNode; cwd?: string; hidden?: boolean; failed?: boolean; onRetry?: () => void; openDisabled?: boolean; forceOpen?: boolean; onToggle?: () => void; onOpenAsSession?: () => void };

export type MainWindowProps = {
  width: number;
  height: number;
  layout?: WindowLayout;
  sidebarPref?: SidebarPref;
  /** The width the user dragged the expanded sidebar to (held to 200 to 400). It does not apply to the rail or in focus mode. */
  sidebarWidth?: number;
  rows?: RowModel[];
  requests?: ApprovalRequest[];
  /** The lane's full card (the app's live one). Left out it is the static card. */
  renderCard?: (request: ApprovalRequest) => ReactNode;
  /** A one-liner in the lane was chosen: go to its session. */
  onJumpRequest?: (request: ApprovalRequest) => void;
  /** A session's last rows as text, for Peek on a row. Left out, rows have no Peek. */
  readScreen?: (id: string) => Promise<string[]>;
  /** Esc closed Peek: focus goes back to the terminal. */
  onPeekClose?: () => void;
  /** The card for the drawer's request. Left out the drawer shows the sample long command. */
  drawerCard?: ReactNode;
  onDrawerClose?: () => void;
  selectedId?: string;
  /** The session on screen, as a list: one id, or none under History. Left out, only `selectedId` is marked. */
  visibleIds?: readonly string[];
  os?: Os;
  /** Wires the Windows minimize, maximize and close buttons. Left out they are drawn but do nothing (stories). */
  windowControls?: { maximized: boolean; onMinimize(): void; onToggleMaximize(): void; onClose(): void };
  terminalSetting?: TerminalThemeSetting;
  /** Shows the session header in focus mode, as hovering the top edge does (stories). */
  headerPeek?: boolean;
  drawer?: boolean;
  /** The prompt line under the terminal; on by default, never in focus mode. */
  composer?: boolean;
  /** What has been typed in the prompt line. */
  composerValue?: string;
  /** Replaces the scripted terminal with the real one. Leave out in stories. `shown` is false for a kept terminal behind the selected one (single window only). */
  renderTerminal?: (row: RowModel, shown?: boolean) => ReactNode;
  /** Ports as `host:port`, for stories: used in place of the rows' own chips, and only when given. */
  ports?: string[];
  /** What the sidebar footer shows; defaults to the scenario's plan and doctor numbers. `null` leaves the footer out (the live app has none to show yet). */
  footer?: SidebarFooter | null;
  /** A click on the sidebar footer's Doctor count. */
  onOpenDoctor?: () => void;
  /** The Menu button in the sidebar footer and at the rail's bottom. Left out, there is none. */
  onOpenMenu?: (button: HTMLElement) => void;
  /** The menu is open: both buttons say so. */
  menuOpen?: boolean;
  /** The needs-you count the window title leads with. The live app passes the one count it also gives the taskbar badge. */
  needsYou?: number;
  /** The 24 px status bar at the foot of the window (the focused session's strip); on by default, never in focus mode. */
  statusBar?: boolean;
  /**
   * The plain shell panel under the session list; on by default in the expanded sidebar. Left as `true` it shows the scripted
   * terminal. An object makes it live: `terminal` replaces the scripted one, `cwd` replaces the focused session's folder,
   * `hidden` is the user's choice to fold it (it stands even with room to spare), `forceOpen` unfolds it when it folded itself
   * for lack of room, `failed` swaps the terminal for a message and a Try again button (`onRetry`), `openDisabled` greys "open
   * as a session", and the other callbacks are its buttons.
   */
  shell?: boolean | ShellOptions;
  /** The user hid the shell panel: header only. */
  shellCollapsed?: boolean;
  /** Takes the place of the session header while it is set (the busy-close question). Shown even where the header is hidden. */
  headerBar?: ReactNode;
  /**
   * A bar for one session (the restart question). It takes the place of the session header, like `headerBar`.
   */
  paneBar?: { id: string; node: ReactNode };
  /** The session header's mode chip becomes a button that gives the session's id and where it is, to open the mode menu. */
  onModeMenu?: (id: string, anchor: { x: number; y: number }) => void;
  /** Keeps the displayed session's mode as text when changing it is unavailable. */
  canChangeMode?: (row: RowModel) => boolean;
  /** A receipt over the lower start corner of the terminal column. Plain markup: `notice` is what gets announced. */
  toast?: ReactNode;
  /** Text for the window's one announcer to say; a new `id` says it again, even if the text is the same. */
  notice?: Notice | null;
  /** F2: the sidebar row whose name is a text box, and what its Enter and its Escape or blur do. */
  renamingId?: string | null;
  onRename?: (id: string, name: string) => void;
  onRenameCancel?: (opts: { refocus: boolean }) => void;
  /** Dragging a sidebar row gives the whole new order. Left out, rows are not draggable. */
  onReorder?: (ids: string[]) => void;
  /** The sidebar groups its rows under a heading per project. Off by default. */
  grouped?: boolean;
  /** The sidebar's drag handle, or its arrow keys, ask for a width. The view state holds it to 200 to 400. Left out, the handle does nothing. */
  onSidebarWidth?: (width: number) => void;
  /** A double click on the sidebar's drag handle asks for the default width. */
  onSidebarReset?: () => void;
  /** A click on a sidebar row or a rail button gives that session's id. Left out, they do nothing (stories). */
  onSelect?: (id: string) => void;
  /** Opens the review of a task row's session (its Review changes button). */
  onReview?: (id: string) => void;
  onSetup?: (id: string) => void;
  /** The selected session's repo run script: Run, or Stop while its tab is going. Both buttons live in the session header. */
  scriptControl?: RunControl | null;
  onRunScript?: () => void;
  onStopScript?: () => void;
  /** The items of a sidebar row's menu, and what choosing one does. Left out, rows have no menu. */
  menuItems?: (id: string) => MenuItem[];
  onMenuRun?: (id: string, itemId: string, anchor: { x: number; y: number }) => void;
  /** The click on the Command palette button, in the top bar and in the rail. */
  onPalette?: () => void;
  /** The click on the sidebar button next to it: fold the sidebar to the rail, or open it. Not called when a narrow window cannot open it. */
  onToggleSidebar?: () => void;
  /** No terminal is selected (Ctrl+Shift+Backspace): no terminal is marked focused, and the window says how to pick one. */
  noneSelected?: boolean;
  /** The keyboard cursor while Ctrl+Shift is held, measured against the window. */
  navCursor?: { rect: Rect; label: string } | null;
  /** The side pane's content. Shown (`paneShown`), it sits beside the session, or over it in a narrow window, and the terminals stay mounted. */
  pane?: ReactNode;
  paneShown?: boolean;
  /** The saved width in pixels, or null for half the room. The window holds no width of its own. */
  paneWidth?: number | null;
  onPaneWidth?: (width: number | null) => void;
  /** The width of 80 terminal columns: the pane stops there, down to its own minimum. */
  terminalMinWidth?: number;
  /**
   * A view that takes the main area (History), in place of the terminal, the empty text and the preview. The sidebar and the bar stay.
   * Where the window draws live terminals they stay mounted behind it, so a session keeps its scrollback.
   */
  mainView?: ReactNode;
};

const icon = (d: string) => (
  <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1">
    <path d={d} />
  </svg>
);

/**
 * The main window composite (docs/PLAN.md deliverable 5): a Windows-style frame around the sidebar or rail, the session header and the terminal.
 *
 * The window draws one terminal: the selected session. Its prompt line sits under it, and the header, the vendor accent and the
 * status bar at the foot of the window all follow that session.
 */
export function MainWindow({
  width,
  height,
  layout = "default",
  sidebarPref,
  sidebarWidth,
  rows = SCENARIO_ROWS,
  requests = SCENARIO_REQUESTS,
  renderCard,
  onJumpRequest,
  readScreen,
  onPeekClose,
  drawerCard,
  onDrawerClose,
  selectedId = "working",
  visibleIds,
  os = "windows",
  windowControls,
  terminalSetting = "follow-app",
  headerPeek: headerPeekProp = false,
  drawer = false,
  composer = true,
  composerValue = "",
  statusBar = true,
  shell = true,
  shellCollapsed = false,
  renderTerminal,
  ports,
  footer = SCENARIO_FOOTER,
  onOpenDoctor,
  onOpenMenu,
  menuOpen,
  needsYou = 0,
  headerBar,
  paneBar,
  onModeMenu,
  canChangeMode,
  toast,
  notice,
  renamingId,
  onRename,
  onRenameCancel,
  onReorder,
  grouped,
  onSelect,
  onReview,
  onSetup,
  scriptControl,
  onRunScript,
  onStopScript,
  menuItems,
  onMenuRun,
  onPalette,
  onToggleSidebar,
  onSidebarWidth,
  onSidebarReset,
  noneSelected = false,
  navCursor = null,
  pane,
  paneShown = false,
  paneWidth = null,
  onPaneWidth,
  terminalMinWidth = 0,
  mainView,
}: MainWindowProps) {
  const pref: SidebarPref = sidebarPref ?? (layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded");
  const { mode, side: sideW, pushed, room: spaceW } = mainRoom(width, { pref, sidebarWidth, drawer });
  // An expanded preference in a window under 960 px is shown as the rail, and the sidebar cannot open there.
  const blocked = sidebarToggleFor(pref, mode) === null;
  const shellOptions: ShellOptions = typeof shell === "object" ? shell : {};
  const expandedW = widthFor("expanded", sidebarWidth);
  // The column's width is an animated number. While it moves it shows its widest end (the sidebar for the rail, the rail for
  // focus mode) at that end's own width, clipped, and swaps to the settled one when the motion ends.
  const sideMotion = useLayoutMotion(mode, sideW);
  const sideAnim = Math.round(sideMotion.value);
  const prevMode = useRef<SidebarMode>(mode);
  const keptMode = useRef<SidebarMode | null>(null);
  const shownMode = sideMotion.moving ? widestMode(mode, prevMode.current, keptMode.current) : mode;
  useEffect(() => {
    keptMode.current = sideMotion.moving ? widestMode(mode, prevMode.current, keptMode.current) : null;
    prevMode.current = mode;
  });
  // No sessions at all is a real state (first run, everything closed): the window shows its empty state, never a missing row.
  const selected: RowModel | null = rows.find((r) => r.id === selectedId) ?? rows[0] ?? null;
  const empty = selected === null;
  const waiting = pendingSessionCount(requests);
  // Focus mode hides the session header until the pointer reaches the top edge. Keyboard users leave focus mode instead.
  const [peeking, setPeeking] = useState(false);
  // A port chip's menu: the window holds it so it sits outside the terminal's [data-agent] element.
  const [portMenu, setPortMenu] = useState<PortMenuRequest | null>(null);
  const chipPorts = ports?.map((p) => ({ port: Number(p.split(":").pop()), url: `http://${p}/` }));
  // Leaving focus mode with the pointer on the bar must not leave the peek set for the next time focus mode starts.
  useEffect(() => {
    if (mode !== "focus") setPeeking(false);
  }, [mode]);
  const showHeader = mode !== "focus" || headerPeekProp || peeking;
  const placement = drawerPlacement(width, mode, DRAWER.default);
  const pillKeys = shortcutLabel(commandById("next-waiting").binding!, os);

  const showComposer = composer && mode !== "focus";
  // A manual CLI is a plain terminal: no prompt line under it.
  const showFull = showComposer && selected !== null && hasPromptLine(selected);
  const showStatus = statusBar && mode !== "focus";
  // The OS title bar and the taskbar name the shown session, like any window named for its document.
  const title = windowTitle(selected, needsYou);
  useEffect(() => {
    if (typeof document !== "undefined") document.title = title;
    // Tauri does not pass document.title on to the window, so the title bar and the taskbar are told as well (a no-op outside the app).
    void setWindowTitle(title).catch(() => {});
  }, [title]);

  const topBar = headerBar ?? paneBar?.node;
  // The sidebar's drag handle: the pointer's distance from the window's start edge is the width; the arrow keys step it by 8 px.
  const bodyRef = useRef<HTMLDivElement>(null);
  const dragSidebar = (e: ReactPointerEvent<HTMLElement>) => {
    const body = bodyRef.current;
    if (!body || !onSidebarWidth || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const rect = body.getBoundingClientRect();
    // In a right-to-left window the sidebar is on the right, so the width is counted from the right edge.
    const rtl = getComputedStyle(body).direction === "rtl";
    onSidebarWidth(clampSidebar(rtl ? rect.right - e.clientX : e.clientX - rect.left));
  };
  const nudgeSidebar = (e: KeyboardEvent<HTMLElement>) => {
    if (!onSidebarWidth || e.ctrlKey || e.altKey || e.metaKey) return;
    const rtl = bodyRef.current !== null && getComputedStyle(bodyRef.current).direction === "rtl";
    const grow = rtl ? "ArrowLeft" : "ArrowRight";
    const shrink = rtl ? "ArrowRight" : "ArrowLeft";
    if (e.key !== grow && e.key !== shrink) return;
    e.preventDefault();
    onSidebarWidth(clampSidebar(sideW + (e.key === grow ? SIDEBAR_STEP : -SIDEBAR_STEP)));
  };
  // The side pane's divider reports the pane's width in pixels and holds none of its own; the saved width comes in as `paneWidth`.
  const mainRef = useRef<HTMLElement>(null);
  const viewing = mainView !== undefined && mainView !== null && mainView !== false;
  const geometry = paneGeometry({ room: spaceW, shown: paneShown && !viewing && !empty, saved: paneWidth, terminalMin: terminalMinWidth });
  const paneOn = geometry.mode !== "hidden";
  const paneCovers = geometry.mode === "cover";
  // The pane moves for 200 ms when its mode changes, and with the sidebar when that switches beside it (paneMotionKey). A drag
  // or a window resize that leaves both alone changes only the target, which the motion follows at once.
  const paneMotion = useLayoutMotion(paneMotionKey(geometry.mode, mode), geometry.pane);
  const layoutMoving = sideMotion.moving || paneMotion.moving;
  // A page in the pane follows the layout from these: the column's inputs, and whether anything is moving. The direction is read
  // once the window is drawn, since it is the document's and a render can't know it.
  const [rtl, setRtl] = useState(false);
  useEffect(() => {
    if (mainRef.current) setRtl(getComputedStyle(mainRef.current).direction === "rtl");
  });
  const paneListed = paneShown && !viewing && !empty;
  const layoutValue = useMemo<Layout>(
    () => ({ inputs: { pref, sidebarWidth, drawer, shown: paneListed, saved: paneWidth, terminalMin: terminalMinWidth, rtl }, moving: layoutMoving }),
    [pref, sidebarWidth, drawer, paneListed, paneWidth, terminalMinWidth, rtl, layoutMoving],
  );
  const roomNow = Math.max(0, width - sideAnim - pushed);
  const paneP = Math.max(0, Math.min(Math.round(paneMotion.value), roomNow));
  // A hide keeps drawing the last pane until its motion ends, so the node is kept (in the same slot, so its state stays).
  const keptPane = useRef<{ node: ReactNode; width: number } | null>(null);
  const paneDrawn = paneOn || paneMotion.moving;
  // The request-details drawer lies over the pane in a narrow window, where a browser tab's page would be drawn above it.
  useCoverPage(drawer && Boolean(drawerCard || requests[0]) && paneDrawn);
  const drawnPane = paneOn ? { node: pane, width: geometry.pane } : keptPane.current;
  useEffect(() => {
    if (paneOn) keptPane.current = { node: pane, width: geometry.pane };
    else if (!paneMotion.moving) keptPane.current = null;
  });
  // During its own motion the pane keeps its settled width and its track cuts it; otherwise it is as wide as its track.
  const paneBodyWidth = paneMotion.moving ? (drawnPane?.width ?? paneP) : paneP;
  const threeTracks = paneMotion.moving || geometry.mode === "beside";
  const dividerLive = geometry.mode === "beside" && !paneMotion.moving;
  const dividerWidth = paneMotion.moving ? (paneP > 0 && paneP < roomNow ? 1 : 0) : 1;
  // Settled, the terminal's track is the pixel width it has always had; while anything moves it takes what the others leave.
  const paneColumns = threeTracks ? `${layoutMoving ? "minmax(0, 1fr)" : `${geometry.terminal}px`} ${dividerWidth}px ${paneP}px` : undefined;
  const dragPane = (e: ReactPointerEvent<HTMLElement>) => {
    const main = mainRef.current;
    if (!onPaneWidth || !main || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const rect = main.getBoundingClientRect();
    if (rect.width <= 0) return;
    const rtl = getComputedStyle(main).direction === "rtl";
    onPaneWidth(draggedWidth({ room: spaceW, terminalMin: terminalMinWidth, fromEnd: rtl ? e.clientX - rect.left : rect.right - e.clientX }));
  };
  const nudgePane = (e: KeyboardEvent<HTMLElement>) => {
    if (!onPaneWidth || e.ctrlKey || e.altKey || e.metaKey) return;
    const rtl = mainRef.current !== null && getComputedStyle(mainRef.current).direction === "rtl";
    // The pane is at the end edge: left-to-right, Left makes it wider and Right narrower; right-to-left mirrors that.
    const grow = rtl ? "ArrowRight" : "ArrowLeft";
    const shrink = rtl ? "ArrowLeft" : "ArrowRight";
    const to = (fromEnd: number) => onPaneWidth(draggedWidth({ room: spaceW, terminalMin: terminalMinWidth, fromEnd }));
    if (e.key === "Home") to(PANE.min);
    else if (e.key === "End") to(geometry.max);
    else if (e.key === grow) to(geometry.pane + PANE.step);
    else if (e.key === shrink) to(geometry.pane - PANE.step);
    else return;
    e.preventDefault();
  };
  return (
    <div className="window" style={{ inlineSize: width, blockSize: height, "--side-w": `${sideAnim}px` } as CSSProperties} data-mode={mode} data-side-moving={sideMotion.moving || undefined} data-layout={layout} data-title={title}>
      <SessionAnnouncer row={selected} needsYou={waiting} notice={notice} />
      {portMenu && <PortMenu request={portMenu} onDone={() => setPortMenu(null)} />}
      {/* data-tauri-drag-region: a bare attribute moves the window only when the press lands on that very element, so buttons and
          the text inside never start a drag; "deep" (the session header's area) also takes its text, while buttons still refuse. */}
      <header className="window__bar" data-tauri-drag-region>
        <div className="window__bar-side" data-tauri-drag-region>
          {/* In focus mode this column has no width: its buttons are not rendered, so keyboard focus can never land on something nobody sees. */}
          {mode !== "focus" && (
            <button type="button" className="window__brand" aria-label="Marshell menu" aria-haspopup="menu">
              <Logo size={22} decorative />
              {mode === "expanded" && <Wordmark height={13} decorative />}
            </button>
          )}
          {mode !== "focus" && (
            <>
              {/* The words go in the expanded bar; the rail's column has room for the icon only. */}
              <button type="button" className="window__palette" data-nav-top="palette" aria-label="Command palette" title={`Search sessions, projects and commands (${shortcutLabel(commandById("palette").binding!, os)})`} onClick={onPalette}>
                <SearchIcon />
                {mode === "expanded" && <span>Command palette</span>}
              </button>
              {/* aria-disabled, not disabled: the button keeps its place in the tab order and says why it does nothing. */}
              <button type="button" className="window__sidebar-toggle" data-nav-top="sidebar" aria-label="Sidebar" aria-expanded={mode === "expanded"} aria-disabled={blocked || undefined} title={blocked ? "Widen the window to open the sidebar" : mode === "expanded" ? "Collapse sidebar" : "Expand sidebar"} onClick={blocked ? undefined : onToggleSidebar}>
                <SidebarIcon open={mode === "expanded"} />
              </button>
            </>
          )}
        </div>
        <div className="window__bar-main" data-tauri-drag-region="deep" onPointerLeave={mode === "focus" ? () => setPeeking(false) : undefined}>
          {topBar ?? (showHeader && selected && <SessionHeader row={selected} ports={chipPorts} onPortMenu={setPortMenu} scriptControl={scriptControl} onRun={onRunScript} onStop={onStopScript} onMode={onModeMenu && (canChangeMode?.(selected) ?? true) ? (anchor) => onModeMenu(selected.id, anchor) : undefined} />)}
          {/* Focus mode: the pointer at the top edge brings the header up, and leaving the bar puts it away. A pointer affordance only; the keyboard leaves focus mode. */}
          {mode === "focus" && !showHeader && <div className="window__peek" aria-hidden="true" onPointerEnter={() => setPeeking(true)} />}
        </div>
        <div className="window__bar-end" data-tauri-drag-region>
          {os === "windows" && (
            <div className="window__controls">
              <button type="button" className="window__control" aria-label="Minimize" onClick={windowControls?.onMinimize}>
                {icon("M0 5h10")}
              </button>
              <button type="button" className="window__control" aria-label={windowControls?.maximized ? "Restore" : "Maximize"} onClick={windowControls?.onToggleMaximize}>
                {icon(windowControls?.maximized ? "M2.5 2.5v-2h7v7h-2M0.5 2.5h7v7h-7z" : "M0.5 0.5h9v9h-9z")}
              </button>
              <button type="button" className="window__control window__control--close" aria-label="Close" onClick={windowControls?.onClose}>
                {icon("M0 0l10 10M10 0L0 10")}
              </button>
            </div>
          )}
        </div>
      </header>

      <div ref={bodyRef} className="window__body">
        <div className="window__side" data-moving={sideMotion.moving || undefined}>
        {shownMode === "expanded" && <Sidebar rows={rows} requests={requests} renderCard={renderCard} onJumpRequest={onJumpRequest} readScreen={readScreen} onPeekClose={onPeekClose} height={height - BAR} width={expandedW} selectedId={selected?.id} visibleIds={visibleIds} os={os} footer={footer ?? undefined} onOpenDoctor={onOpenDoctor} onOpenMenu={onOpenMenu} menuOpen={menuOpen} shell={shell ? { cwd: shellOptions.cwd ?? selected?.project, shellName: shellNameFor(os), collapsed: shellCollapsed || shellOptions.hidden === true, os, terminal: shellOptions.terminal, failed: shellOptions.failed, onRetry: shellOptions.onRetry, openDisabled: shellOptions.openDisabled, forceOpen: shellOptions.forceOpen, onToggle: shellOptions.onToggle, onOpenAsSession: shellOptions.onOpenAsSession } : undefined} renamingId={renamingId} onRename={onRename} onRenameCancel={onRenameCancel} onReorder={onReorder} onSelect={onSelect} onReview={onReview} onSetup={onSetup} menuItems={menuItems} onMenuRun={onMenuRun} grouped={grouped} />}
        {shownMode === "rail" && <Rail rows={rows} needsYou={waiting} selectedId={selected?.id} visibleIds={visibleIds} os={os} onSelect={onSelect} onOpenMenu={onOpenMenu} menuOpen={menuOpen} />}
        </div>
        {mode === "expanded" && (
          <div
            className="window__side-handle"
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize the sidebar"
            aria-valuemin={SIDEBAR.min}
            aria-valuemax={SIDEBAR.max}
            aria-valuenow={sideW}
            tabIndex={onSidebarWidth ? 0 : undefined}
            onPointerDown={onSidebarWidth ? (e) => e.button === 0 && e.currentTarget.setPointerCapture(e.pointerId) : undefined}
            onPointerMove={onSidebarWidth ? dragSidebar : undefined}
            onKeyDown={onSidebarWidth ? nudgeSidebar : undefined}
            onDoubleClick={onSidebarReset}
          />
        )}

        <div className="window__center">
        {/* The terminal is slot 0 to the keyboard cursor. */}
        <FitHoldContext.Provider value={layoutMoving}>
        <LayoutContext.Provider value={layoutValue}>
        <main
          ref={mainRef}
          className="window__main"
          tabIndex={-1}
          data-placement={drawer && !paneDrawn ? placement : undefined}
          data-nav-pane={!viewing && !empty ? 0 : undefined}
          data-pane={paneOn ? geometry.mode : paneMotion.moving ? "hiding" : undefined}
          style={paneColumns ? { gridTemplateColumns: paneColumns } : undefined}
        >
          {empty ? (
            viewing ? null : (
              <div className="window__empty">
                No sessions yet. Start one with {shortcutLabel(commandById("launcher").binding!, os)}.
              </div>
            )
          ) : renderTerminal ? (
            // Under a main view every terminal stays mounted and none is on screen.
            <KeptTerminals rows={rows} selectedId={selected.id} render={renderTerminal} suspended={(paneCovers && !paneMotion.moving) || viewing} />
          ) : viewing ? null : (
            <ScriptedTerminal setting={terminalSetting} label={`Terminal for ${selected.name}`} {...(isTool(selected.agent) ? { script: shellSession({ cwd: selected.project }), size: SHELL_SIZE } : {})} />
          )}

          {viewing && mainView}

          {threeTracks &&
            (dividerLive ? (
              <div
                className="pane__divider"
                role="separator"
                aria-orientation="vertical"
                aria-label="Resize the side pane"
                aria-valuemin={PANE.min}
                aria-valuemax={geometry.max}
                aria-valuenow={geometry.pane}
                tabIndex={0}
                onPointerDown={(e) => e.button === 0 && e.currentTarget.setPointerCapture(e.pointerId)}
                onPointerMove={dragPane}
                onKeyDown={nudgePane}
                onDoubleClick={() => onPaneWidth?.(null)}
              />
            ) : (
              // While the pane moves the divider holds its track as a line nobody can reach.
              <div className="pane__divider" data-still="" aria-hidden="true" />
            ))}
          {/* The slot cuts the pane to its track while it moves; the body keeps the pane's settled width, pinned to the end edge. */}
          {paneDrawn && (
            <div className="window__pane" data-moving={paneMotion.moving || undefined}>
              <div className="window__pane-body" style={{ inlineSize: paneBodyWidth }}>
                {drawnPane?.node}
              </div>
            </div>
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

          {drawer && (drawerCard || requests[0]) && (
            <aside
              className="drawer"
              data-placement={placement}
              style={{ inlineSize: DRAWER.default }}
              aria-label="Request details"
              onKeyDown={(e) => {
                if (e.key === "Escape" && onDrawerClose) {
                  e.stopPropagation();
                  onDrawerClose();
                }
              }}
            >
              {onDrawerClose && (
                <button type="button" className="drawer__close" onClick={onDrawerClose}>
                  Close details
                </button>
              )}
              {drawerCard ?? <ApproveCard request={{ ...requests[0]!, detail: DRAWER_DETAIL }} expanded />}
            </aside>
          )}
        </main>
        </LayoutContext.Provider>
        </FitHoldContext.Provider>
        {toast}
        {showFull && selected && (
          <div className="window__composer" style={{ blockSize: COMPOSER.full }}>
            <Composer row={selected} value={composerValue} os={os} />
          </div>
        )}
        {showStatus && (
          <div className="window__status" style={{ blockSize: STATUS_BAR_HEIGHT }}>
            {noneSelected ? <p className="window__status-none">{NO_SELECTION_TEXT}</p> : <StatusBar row={selected} plan={isTool(selected?.agent ?? "") ? undefined : SCENARIO_PLAN} ports={ports} os={os} />}
          </div>
        )}
        </div>
      </div>
      {/* With no status bar to say it (the live window), the same line sits where the cursor's hint does, until the hint takes its place. */}
      {noneSelected && !showStatus && !navCursor && (
        <p className="nav-hint" aria-hidden="true">
          <span className="nav-hint__keys">{NO_SELECTION_TEXT}</span>
        </p>
      )}
      {navCursor && <NavCursor rect={navCursor.rect} label={navCursor.label} os={os} />}
    </div>
  );
}
