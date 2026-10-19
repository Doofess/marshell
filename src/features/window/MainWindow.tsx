import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import type { MenuItem } from "../../components/ContextMenu/ContextMenu";
import { setTitle as setWindowTitle } from "../../lib/window/appWindow";
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
import { COMPOSER, DRAWER, STATUS_BAR_HEIGHT, SIDEBAR, drawerPlacement, sidebarMode, sidebarWidth as widthFor, type SidebarPref } from "./layout";
import { LayoutIcon, LayoutMenu } from "./LayoutMenu";
import { SearchIcon } from "./SearchIcon";
import { SidebarIcon } from "./SidebarIcon";
import { isTool } from "../../components/AgentMark/kinds";
import { hasPromptLine } from "../sidebar/promptLine";
import type { Rect } from "../../lib/navigation/types";
import { KeptTerminals } from "./keptTerminals";
import { NavCursor, NO_SELECTION_TEXT } from "./NavCursor";
import { Rail } from "./Rail";
import { SCENARIO_FOOTER, SCENARIO_PLAN, SCENARIO_PORTS, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import type { Notice } from "./announce";
import { SessionAnnouncer } from "./SessionAnnouncer";
import { SessionHeader } from "./SessionHeader";
import { windowTitle } from "./windowTitle";
import { Sidebar, type SidebarFooter } from "./Sidebar";
import { shellNameFor } from "./ShellPanel";
import { ARRANGEMENTS, chooseArrangement, maxPanes, type Arrangement, type PaneCount, type Ratios } from "./splitLayout";
import { PREVIEW_RATIO, clampPreviewRatio, previewLayout, previewTracks } from "./previewLayout";
import { SplitView } from "./SplitView";
import { focusedIndex, sidebarToggleFor, type ViewAction } from "./viewState";
import "./MainWindow.css";

/** The drawer shows the request's full payload; the 40-line command is the long one. */
const DRAWER_DETAIL = APPROVALS.longCommand.detail;
const BAR = 40;
/** How far one arrow key press moves the sidebar's edge. */
const SIDEBAR_STEP = 8;
const clampSidebar = (w: number) => Math.round(Math.min(SIDEBAR.max, Math.max(SIDEBAR.min, w)));

export type WindowLayout = "default" | "split" | "focus" | "rail";
/** What makes the sidebar's shell panel live (see `MainWindowProps.shell`). */
export type ShellOptions = { terminal?: ReactNode; cwd?: string; hidden?: boolean; failed?: boolean; onRetry?: () => void; openDisabled?: boolean; forceOpen?: boolean; onToggle?: () => void; onOpenInPane?: () => void };

export type MainWindowProps = {
  width: number;
  height: number;
  layout?: WindowLayout;
  sidebarPref?: SidebarPref;
  /** The width the user dragged the expanded sidebar to (held to 200 to 400). It does not apply to the rail or in focus mode. */
  sidebarWidth?: number;
  rows?: RowModel[];
  requests?: ApprovalRequest[];
  selectedId?: string;
  /** Every session on screen (the panes' sessions). Left out, only `selectedId` is marked. */
  visibleIds?: readonly string[];
  os?: Os;
  /** Wires the Windows minimize, maximize and close buttons. Left out they are drawn but do nothing (stories). */
  windowControls?: { maximized: boolean; onMinimize(): void; onToggleMaximize(): void; onClose(): void };
  terminalSetting?: TerminalThemeSetting;
  /** Shows the session header in focus mode, as hovering the top edge does (stories). */
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
  /**
   * Split view from the window's saved layout: one entry per pane, in pane order (`null` is an empty pane). When set, it replaces the
   * selected-first ordering and `splitCount`; panes beyond what the window can hold are hidden, not dropped.
   */
  paneIds?: (string | null)[];
  /** The layout menu gave a pane count and an arrangement (`null` is automatic). Left out, the menu's choices do nothing (stories). */
  onLayout?: (count: PaneCount, arrangement: Arrangement | null) => void;
  /** A pane was clicked, zoomed, removed, filled from an empty pane, or a divider moved: the change to the view. */
  onPane?: (action: ViewAction) => void;
  /** The layout button, a click outside the menu, or Escape asks for the menu to open or close. */
  onLayoutMenu?: (open: boolean) => void;
  /** How many panes the window really shows (fewer than the layout holds when it is small); the focus follows the shown panes. */
  onVisible?: (count: number) => void;
  /** How many panes this window has room for now (up to four), shown or not. A split past it would only add a pane out of sight. */
  onFit?: (count: number) => void;
  /** The prompt lines (one under a single terminal, one inside every pane in split view); on by default, never in focus mode. */
  composer?: boolean;
  /** What has been typed in the single window's prompt line. */
  composerValue?: string;
  /** What has been typed in each pane's own prompt line in split view, by session id. */
  composerValues?: Record<string, string>;
  /** Replaces the scripted terminal of every pane with the real one. Leave out in stories. `shown` is false for a kept terminal behind the selected one (single window only). */
  renderTerminal?: (row: RowModel, shown?: boolean) => ReactNode;
  /** The ports the session header lists; defaults to the scenario's. */
  ports?: string[];
  /** What the sidebar footer shows; defaults to the scenario's plan and doctor numbers. `null` leaves the footer out (the live app has none to show yet). */
  footer?: SidebarFooter | null;
  /** The 24 px status bar at the foot of the window (the focused session's strip); on by default, never in focus mode. */
  statusBar?: boolean;
  /**
   * The plain shell panel under the session list; on by default in the expanded sidebar. Left as `true` it shows the scripted
   * terminal. An object makes it live: `terminal` replaces the scripted one, `cwd` replaces the focused session's folder,
   * `hidden` is the user's choice to fold it (it stands even with room to spare), `forceOpen` unfolds it when it folded itself
   * for lack of room, `failed` swaps the terminal for a message and a Try again button (`onRetry`), `openDisabled` greys "open
   * in a pane", and the other callbacks are its buttons.
   */
  shell?: boolean | ShellOptions;
  /** The user hid the shell panel: header only. */
  shellCollapsed?: boolean;
  /** Takes the place of the session header while it is set (the busy-close question). Shown even where the header is hidden. */
  headerBar?: ReactNode;
  /**
   * A bar for one session (the restart question). With two or more panes on screen it replaces that pane's header; otherwise it
   * takes the place of the session header, like `headerBar`.
   */
  paneBar?: { id: string; node: ReactNode };
  /** The session header's mode chip becomes a button that gives the session's id and where it is, to open the mode menu. */
  onModeMenu?: (id: string, anchor: { x: number; y: number }) => void;
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
  /** Ctrl+click (Cmd on macOS) or Ctrl+Space on a session: add or remove its pane. */
  onToggle?: (id: string) => void;
  /** Opens the review of a task row's session (its Review changes button). */
  onReview?: (id: string) => void;
  onSetup?: (id: string) => void;
  /** The items of a sidebar row's menu, and what choosing one does. Left out, rows have no menu. */
  menuItems?: (id: string) => MenuItem[];
  onMenuRun?: (id: string, itemId: string, anchor: { x: number; y: number }) => void;
  /** The click on the Command palette button, in the top bar and in the rail. */
  onPalette?: () => void;
  /** The click on the sidebar button next to it: fold the sidebar to the rail, or open it. Not called when a narrow window cannot open it. */
  onToggleSidebar?: () => void;
  /** No terminal is selected (Ctrl+Shift+Backspace): no pane is marked focused, and the window says how to pick one. */
  noneSelected?: boolean;
  /** The keyboard cursor while Ctrl+Shift is held, measured against the window. */
  navCursor?: { rect: Rect; label: string } | null;
  /**
   * The preview pane. Present, it sits beside the focused session (or over it, in a narrow window) and the terminals stay mounted.
   * A split view shows only the focused pane's session while it is open; closing the preview brings the split back as it was.
   */
  preview?: ReactNode;
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
  sidebarWidth,
  rows = SCENARIO_ROWS,
  requests = SCENARIO_REQUESTS,
  selectedId = "working",
  visibleIds,
  os = "windows",
  windowControls,
  terminalSetting = "follow-app",
  headerPeek: headerPeekProp = false,
  drawer = false,
  splitCount = 2,
  arrangement,
  ratios,
  activePane = 0,
  zoomedPane,
  emptySlot = false,
  layoutMenu = false,
  paneIds,
  onLayout,
  onPane,
  onLayoutMenu,
  onVisible,
  onFit,
  composer = true,
  composerValue = "",
  composerValues,
  statusBar = true,
  shell = true,
  shellCollapsed = false,
  renderTerminal,
  ports = SCENARIO_PORTS,
  footer = SCENARIO_FOOTER,
  headerBar,
  paneBar,
  onModeMenu,
  toast,
  notice,
  renamingId,
  onRename,
  onRenameCancel,
  onReorder,
  grouped,
  onSelect,
  onToggle,
  onReview,
  onSetup,
  menuItems,
  onMenuRun,
  onPalette,
  onToggleSidebar,
  onSidebarWidth,
  onSidebarReset,
  noneSelected = false,
  navCursor = null,
  preview,
}: MainWindowProps) {
  const pref: SidebarPref = sidebarPref ?? (layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded");
  const mode = sidebarMode(width, pref);
  // An expanded preference in a window under 960 px is shown as the rail, and the sidebar cannot open there.
  const blocked = sidebarToggleFor(pref, mode) === null;
  const shellOptions: ShellOptions = typeof shell === "object" ? shell : {};
  const sideW = widthFor(mode, sidebarWidth);
  // No sessions at all is a real state (first run, everything closed): the window shows its empty state, never a missing row.
  const selected: RowModel | null = rows.find((r) => r.id === selectedId) ?? rows[0] ?? null;
  const empty = selected === null;
  const waiting = pending(requests).length;
  // Focus mode hides the session header until the pointer reaches the top edge; the pane headers stay. Keyboard users leave focus mode instead.
  const [peeking, setPeeking] = useState(false);
  // Leaving focus mode with the pointer on the bar must not leave the peek set for the next time focus mode starts.
  useEffect(() => {
    if (mode !== "focus") setPeeking(false);
  }, [mode]);
  const showHeader = mode !== "focus" || headerPeekProp || peeking;
  const placement = drawerPlacement(width, mode, DRAWER.default);
  const pillKeys = shortcutLabel(commandById("next-waiting").binding!, os);

  // Split view: the selected session first, then the others in order, never the same one twice.
  const split = layout === "split";
  const showComposer = composer && mode !== "focus";
  // A manual CLI is a plain terminal: no full prompt line under a single one, and no compact line in its pane.
  const showFull = showComposer && !split && selected !== null && hasPromptLine(selected);
  const showStatus = statusBar && mode !== "focus";
  const pushed = drawer && placement === "push" ? DRAWER.default : 0;
  // The status bar always takes its 24 px. A single terminal also gives up the full prompt line; in split view each pane's own
  // prompt line is counted inside the pane math (splitLayout), not here.
  // With `paneIds` the window's saved layout decides which sessions are in which pane, empty panes included; a pane whose
  // session is not in the list (closed, not loaded yet) is an empty pane. Without it, the selected session comes first.
  const requested = paneIds ? paneIds.length : Math.min(splitCount, rows.length);
  const ordered = selected ? [selected, ...rows.filter((r) => r.id !== selected.id)] : [];
  const wanted: (RowModel | null)[] = paneIds ? paneIds.map((id) => (id === null ? null : (rows.find((r) => r.id === id) ?? null))) : ordered.slice(0, requested);
  const lineH = split && showComposer && wanted.some((r) => r !== null && hasPromptLine(r)) ? COMPOSER.compact : 0;
  const space = { w: width - sideW - pushed, h: height - BAR - (showStatus ? STATUS_BAR_HEIGHT : 0) - (showFull ? COMPOSER.full : 0) };
  // Four only where four are usable: a window too small for the count shows fewer, and the rest keep running.
  const visible = split ? Math.min(requested, maxPanes(space, lineH)) : 1;
  const shown: (RowModel | null)[] = wanted.slice(0, visible);
  if (!paneIds && emptySlot && shown.length > 1) shown[shown.length - 1] = null;
  // An empty pane offers every session that has no pane, the hidden panes' sessions excluded.
  const choices = rows.filter((r) => !(paneIds ? wanted : shown).some((s) => s?.id === r.id));
  // Zoom makes its pane the focused one, so the header, the accent bar and the prompt bar can never disagree about which pane is in play.
  const zoomIdx = split && zoomedPane !== undefined && shown[zoomedPane] != null ? zoomedPane : undefined;
  const focusedIdx = focusedIndex(shown, activePane, zoomIdx);
  const focused = shown[focusedIdx] ?? null;
  const active = split ? (focused ?? selected) : selected;
  const paneCount = (split && !empty ? Math.max(1, shown.length) : 1) as PaneCount;
  // An arrangement picked for another number of panes (the window shrank, or fewer sessions exist) gives way to automatic.
  const chosen = arrangement && ARRANGEMENTS[zoomIdx !== undefined ? 1 : paneCount]?.includes(arrangement) ? arrangement : undefined;
  const arranged = split && !empty ? (zoomIdx !== undefined ? "single" : (chosen ?? chooseArrangement(paneCount, space, {}, lineH))) : "single";
  const menuCount = (split && !empty ? (zoomIdx !== undefined ? 1 : paneCount) : 1) as PaneCount;
  const hiddenCount = split ? requested - shown.length : 0;
  // The OS title bar and the taskbar name the focused session, like any window named for its document.
  const title = windowTitle(active);
  useEffect(() => {
    if (typeof document !== "undefined") document.title = title;
    // Tauri does not pass document.title on to the window, so the title bar and the taskbar are told as well (a no-op outside the app).
    void setWindowTitle(title).catch(() => {});
  }, [title]);

  const shownCount = shown.length;
  // With two or more panes on screen each pane's header names its session, so the top bar names none: above the first pane it read as that pane's title.
  const manyPanes = split && !empty && zoomIdx === undefined && shownCount > 1;
  // A pane bar belongs to its pane's header when that pane is one of several; alone, or for a session with no pane here, it is the top bar's.
  const barInPane = paneBar !== undefined && manyPanes && shown.some((r) => r?.id === paneBar.id);
  const topBar = headerBar ?? (paneBar && !barInPane ? paneBar.node : undefined);
  useEffect(() => {
    onVisible?.(shownCount);
  }, [onVisible, shownCount]);
  const fitCount = maxPanes(space, lineH);
  useEffect(() => {
    onFit?.(fitCount);
  }, [onFit, fitCount]);

  // The layout menu closes on Escape (the focus goes back to its button) and on a press anywhere outside it.
  const layoutRoot = useRef<HTMLDivElement>(null);
  const layoutButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!layoutMenu || !onLayoutMenu) return;
    const away = (e: PointerEvent) => {
      if (!(e.target instanceof Node && layoutRoot.current?.contains(e.target))) onLayoutMenu(false);
    };
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [layoutMenu, onLayoutMenu]);
  const layoutKeys = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "Escape" || !layoutMenu || !onLayoutMenu) return;
    e.stopPropagation();
    onLayoutMenu(false);
    layoutButton.current?.focus();
  };
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
  // The preview's divider: the session's share of the main area. Not saved; every opening starts at half.
  const mainRef = useRef<HTMLElement>(null);
  const [previewRatio, setPreviewRatio] = useState<number>(PREVIEW_RATIO.default);
  const previewing = !empty && preview !== undefined && preview !== null && preview !== false;
  const previewFull = previewing && previewLayout(space.w) === "full";
  const dragPreview = (e: ReactPointerEvent<HTMLElement>) => {
    const main = mainRef.current;
    if (!main || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const rect = main.getBoundingClientRect();
    if (rect.width <= 0) return;
    const rtl = getComputedStyle(main).direction === "rtl";
    setPreviewRatio(clampPreviewRatio((rtl ? rect.right - e.clientX : e.clientX - rect.left) / rect.width));
  };
  const nudgePreview = (e: KeyboardEvent<HTMLElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const rtl = mainRef.current !== null && getComputedStyle(mainRef.current).direction === "rtl";
    const grow = rtl ? "ArrowLeft" : "ArrowRight";
    const shrink = rtl ? "ArrowRight" : "ArrowLeft";
    if (e.key !== grow && e.key !== shrink) return;
    e.preventDefault();
    setPreviewRatio((r) => clampPreviewRatio(r + (e.key === grow ? PREVIEW_RATIO.step : -PREVIEW_RATIO.step)));
  };
  // The pane clicks go out as view actions. Zooming makes the pane the focused one first, so it is the pane the zoom applies to.
  const paneHandlers = onPane && {
    onFocus: (index: number) => onPane({ type: "focusPane", index }),
    onZoom: (index: number) => {
      onPane({ type: "focusPane", index });
      onPane({ type: "zoom" });
    },
    onRemove: (index: number) => onPane({ type: "remove", index }),
    onPick: (index: number, tabId: string) => onPane({ type: "assign", index, tabId }),
    onRatios: (r: Ratios) => onPane({ type: "ratios", ratios: r }),
  };
  // What the menu shows as "asked for" is the number of panes, hidden ones included; a zoomed or single window asks for one.
  const asked = split && zoomIdx === undefined ? requested : 1;

  return (
    <div className="window" style={{ inlineSize: width, blockSize: height, "--side-w": `${sideW}px` } as CSSProperties} data-mode={mode} data-layout={layout} data-title={title}>
      <SessionAnnouncer row={active} needsYou={waiting} notice={notice} />
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
          {topBar ?? (showHeader && !manyPanes && active && <SessionHeader row={active} ports={ports} onMode={onModeMenu && ((anchor) => onModeMenu(active.id, anchor))} />)}
          {/* Focus mode: the pointer at the top edge brings the header up, and leaving the bar puts it away. A pointer affordance only; the keyboard leaves focus mode. */}
          {mode === "focus" && !showHeader && !manyPanes && <div className="window__peek" aria-hidden="true" onPointerEnter={() => setPeeking(true)} />}
        </div>
        <div className="window__bar-end" data-tauri-drag-region>
          <div ref={layoutRoot} className="window__layout" data-tauri-drag-region onKeyDown={layoutKeys}>
            <button
              ref={layoutButton}
              type="button"
              className="window__control window__control--menu"
              data-nav-top="layout"
              aria-label="Split layout"
              aria-haspopup="dialog"
              aria-expanded={layoutMenu}
              title="Split layout: up to four terminals on screen"
              onClick={() => onLayoutMenu?.(!layoutMenu)}
            >
              <LayoutIcon count={menuCount} arrangement={arranged} size={18} />
            </button>
            {layoutMenu && (
              <div className="window__popover">
                <LayoutMenu
                  count={menuCount}
                  requested={asked}
                  max={maxPanes(space, lineH)}
                  arrangement={arranged}
                  auto={chosen === undefined}
                  os={os}
                  onCount={onLayout && ((n) => onLayout(n, chosen ?? null))}
                  onArrange={onLayout && ((a) => onLayout(asked as PaneCount, a))}
                  focusOnOpen={onLayoutMenu !== undefined}
                />
              </div>
            )}
          </div>
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
        {mode === "expanded" && <Sidebar rows={rows} requests={requests} height={height - BAR} width={sideW} selectedId={selected?.id} visibleIds={visibleIds} os={os} footer={footer ?? undefined} shell={shell ? { cwd: shellOptions.cwd ?? active?.project, shellName: shellNameFor(os), collapsed: shellCollapsed || shellOptions.hidden === true, os, terminal: shellOptions.terminal, failed: shellOptions.failed, onRetry: shellOptions.onRetry, openDisabled: shellOptions.openDisabled, forceOpen: shellOptions.forceOpen, onToggle: shellOptions.onToggle, onOpenInPane: shellOptions.onOpenInPane } : undefined} renamingId={renamingId} onRename={onRename} onRenameCancel={onRenameCancel} onReorder={onReorder} onSelect={onSelect} onToggle={onToggle} onReview={onReview} onSetup={onSetup} menuItems={menuItems} onMenuRun={onMenuRun} grouped={grouped} />}
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
        {mode === "rail" && <Rail rows={rows} needsYou={waiting} selectedId={selected?.id} visibleIds={visibleIds} os={os} onSelect={onSelect} onToggle={onToggle} />}

        <div className="window__center">
        {/* A single terminal is slot 0 to the keyboard cursor; in split view each pane is its own slot. */}
        <main
          ref={mainRef}
          className="window__main"
          tabIndex={-1}
          data-placement={drawer && !previewing ? placement : undefined}
          data-nav-pane={!empty && (!split || previewing) ? 0 : undefined}
          data-preview={previewing ? (previewFull ? "full" : "beside") : undefined}
          style={previewing && !previewFull ? { gridTemplateColumns: previewTracks(previewRatio) } : undefined}
        >
          {empty ? (
            <div className="window__empty">
              No sessions yet. Start one with {shortcutLabel(commandById("launcher").binding!, os)}.
            </div>
          ) : split && !previewing ? (
            <SplitView panes={shown} size={{ w: space.w, h: space.h }} activeIndex={noneSelected ? -1 : focusedIdx} hidden={hiddenCount} arrangement={chosen} ratios={ratios} zoomed={zoomIdx} choices={choices} composerValues={composerValues} composers={showComposer} setting={terminalSetting} os={os} renderTerminal={renderTerminal} paneBar={barInPane ? paneBar : undefined} {...paneHandlers} />
          ) : renderTerminal ? (
            // While the preview is open in a split view, the focused pane's session is the one beside it; the others stay mounted behind.
            <KeptTerminals rows={rows} selectedId={previewing && split ? (active?.id ?? selected.id) : selected.id} render={renderTerminal} suspended={previewFull} />
          ) : (
            <ScriptedTerminal setting={terminalSetting} label={`Terminal for ${selected.name}`} {...(isTool(selected.agent) ? { script: shellSession({ cwd: selected.project }), size: SHELL_SIZE } : {})} />
          )}

          {previewing && !previewFull && (
            <div
              className="split__handle preview__divider"
              data-orientation="vertical"
              role="separator"
              aria-orientation="vertical"
              aria-label="Resize the preview"
              aria-valuemin={Math.round(PREVIEW_RATIO.min * 100)}
              aria-valuemax={Math.round(PREVIEW_RATIO.max * 100)}
              aria-valuenow={Math.round(clampPreviewRatio(previewRatio) * 100)}
              tabIndex={0}
              onPointerDown={(e) => e.button === 0 && e.currentTarget.setPointerCapture(e.pointerId)}
              onPointerMove={dragPreview}
              onKeyDown={nudgePreview}
              onDoubleClick={() => setPreviewRatio(PREVIEW_RATIO.default)}
            />
          )}
          {previewing && preview}

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
        {toast}
        {showFull && selected && (
          <div className="window__composer" style={{ blockSize: COMPOSER.full }}>
            <Composer row={selected} variant="full" value={composerValue} os={os} />
          </div>
        )}
        {showStatus && (
          <div className="window__status" style={{ blockSize: STATUS_BAR_HEIGHT }}>
            {noneSelected ? <p className="window__status-none">{NO_SELECTION_TEXT}</p> : <StatusBar row={split ? focused : selected} plan={isTool((split ? focused : selected)?.agent ?? "") ? undefined : SCENARIO_PLAN} ports={ports} os={os} />}
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
