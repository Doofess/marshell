import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import { ApiError } from "../../lib/api/client";
import { sessions } from "../../lib/api/sessions";
import { copyRecordingPath } from "./recording";
import { tasksApi } from "../../lib/api/tasks";
import { cancelsCloseBar, commandShortcut, sessionShortcut, type SessionShortcut } from "../../lib/appShortcuts";
import { osOfPlatform } from "../../lib/keymap";
import { paletteSessions } from "../palette/sessions";
import { ProfilesDialog } from "../settings/ProfilesPage";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import type { RowModel } from "../sidebar/types";
import { AppearanceDialog } from "../appearance/AppearanceDialog";
import { useAppearance } from "../appearance/AppearanceProvider";
import { terminalThemeOf, wtFont } from "../appearance/terminalSettings";
import { changesFor, isTextBox, keyBlocked, markTasks, missingTask, paletteHidden, resolveTarget, reviewAgentName, reviewKeyDown, reviewTarget, taskRefresher, type ReviewTarget } from "../tasks/reviewOpen";
import { ReviewSheet } from "../tasks/ReviewSheet";
import { reviewApi } from "../tasks/reviewRun";
import { jumpWords } from "../terminal/promptMarks";
import { resolveTerminalFont } from "../terminal/terminalFont";
import { TerminalView, type TerminalThemeProp } from "../terminal/TerminalView";
import { sidebarMode } from "../window/layout";
import { MainWindow } from "../window/MainWindow";
import { PANE_LIMIT, type Arrangement } from "../window/splitLayout";
import { useViewState } from "../window/useViewState";
import { adoptTarget, fillsEmptyPane, focusedIndex, railToggle, ratiosOf, roomForPane, sidebarPrefOf } from "../window/viewState";
import { CloseBar } from "./CloseBar";
import { launcherOpens } from "./launcherOpens";
import { LiveLauncher } from "./LiveLauncher";
import { LivePalette } from "./LivePalette";
import { LiveShortcuts } from "./LiveShortcuts";
import { ResumeBar } from "./ResumeBar";
import { resumeCommand } from "./resumeCommand";
import { holdsResume, resumeDisabledReason, resumeStateOf } from "./resumeState";
import { SessionActions, closeBarText, tabIdOfNumber } from "./sessionActions";
import { navPanesOf, useNavigation } from "./useNavigation";
import type { NavEffect } from "../../lib/navigation/types";
import { Toast } from "./Toast";
import { pruneClosed, without, type StartingTexts } from "./startingTexts";
import { SHELL_FAILED } from "../window/ShellPanel";
import { focusShellTerminal, shellAutoFolded, shellTerminalFocused } from "./shellPanelFocus";
import { ENDED_PHRASE, lastSegment } from "./toRow";
import { useSessions } from "./useSessions";

/** The size a new session starts at until a terminal has been measured. */
const DEFAULT_SIZE = { cols: 120, rows: 30 };
/** The core's id for a session started outside any saved shell profile; it never has a profile to lose. */
const CUSTOM_PROFILE = "custom";
/** What the announcer says when the shell panel is asked for where it is not drawn: the rail, focus mode, or a window under 960 px. */
const SHELL_PANEL_ELSEWHERE = "The shell panel is in the expanded sidebar.";
/** What the announcer says when a split is asked for and the window has no room to show another pane. */
const NO_PANE_ROOM = "No room for another pane.";

/** The size of the element, kept up to date with a ResizeObserver. Starts at the window's size. */
function useElementSize() {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: window.innerWidth, height: window.innerHeight });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return { ref, ...size };
}

/** The real app: the main window on the core's live session list, and the launcher that adds to it. */
export function LiveApp() {
  const { endpoint, profiles, rows, infos, selectedId, loaded, select, reloadProfiles, error } = useSessions();
  // The window's saved layout: which sessions are in which pane. `undefined` until the list has answered, so an empty first list is never "everything closed".
  const [view, dispatch, , settled] = useViewState(endpoint, loaded ? infos.map((i) => i.tab_id) : undefined);
  const viewRef = useRef(view);
  viewRef.current = view;
  // The session the window is about is the one in the focused pane; the store's choice only stands in for an empty pane.
  // A small window shows fewer panes than the layout holds; the focus is the pane it shows, never one out of sight (MainWindow reports how many).
  const [shownCount, setShownCount] = useState<number | undefined>(undefined);
  // How many panes the window has room for (the most it could show). Both counts are also kept in refs, for the callbacks below.
  const [fitCount, setFitCount] = useState<number | undefined>(undefined);
  const shownRef = useRef(shownCount);
  shownRef.current = shownCount;
  const fitRef = useRef(fitCount);
  fitRef.current = fitCount;
  const shownPanes = view.panes.slice(0, shownCount ?? view.panes.length);
  const focusIdx = focusedIndex(shownPanes, view.active, view.zoomed);
  const focusIdxRef = useRef(focusIdx);
  focusIdxRef.current = focusIdx;
  const focusedId = view.panes[focusIdx] ?? selectedId;
  // Going to a session is two things: the store's selection and the view's, where it focuses the pane showing it or fills the focused one.
  const selectRef = useRef(select);
  selectRef.current = select;
  const pick = useCallback(
    (id: string) => {
      selectRef.current(id);
      // The view is told how many panes are on screen, so the session never lands in one that is not.
      dispatch({ type: "select", tabId: id, visible: shownRef.current });
    },
    [dispatch],
  );
  const [layoutMenu, setLayoutMenu] = useState(false);
  const { ref, width, height } = useElementSize();
  const { appearance, wtFiles } = useAppearance();
  // A session's theme is the setting with its profile's override. One object per override, kept until the setting changes,
  // so a terminal restyles only when its theme really does (the session list updates far more often).
  const themeFor = useMemo(() => {
    const cache = new Map<string, TerminalThemeProp>();
    return (override: string | undefined) => {
      const key = override ?? "";
      let theme = cache.get(key);
      if (!theme) cache.set(key, (theme = terminalThemeOf(appearance, override)));
      return theme;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only the terminal parts of the appearance count.
  }, [appearance.terminal_theme, appearance.imported_themes, appearance.contrast_protection]);
  const overrideOf = useCallback(
    (tabId: string) => {
      const profileId = infos.find((i) => i.tab_id === tabId)?.profile_id;
      return profiles.find((p) => p.id === profileId)?.terminal_theme;
    },
    [infos, profiles],
  );
  const wt = useMemo(() => (wtFiles ? wtFont(wtFiles) : null), [wtFiles]);
  const font = resolveTerminalFont(appearance.terminal_font, wt);
  const terminalFont = useMemo(() => ({ family: font.family, size: font.size }), [font.family, font.size]);
  const [launcherOpen, setLauncherOpen] = useState(false);
  // The command palette, or the shell profiles page opened from it, over the window.
  const [overlay, setOverlay] = useState<"palette" | "profiles" | "appearance" | "shortcuts" | null>(null);
  // What the profiles page and the prompt jumps say aloud through the window's announcer. Negative ids never meet a toast's.
  const [notice, setNotice] = useState<{ id: number; text: string } | null>(null);
  const noticeCount = useRef(0);
  const announce = useCallback((text: string) => setNotice({ id: -++noticeCount.current, text }), []);
  // A prompt jump asked of one session's terminal; each token is one jump.
  const [promptJump, setPromptJump] = useState<{ token: number; by: -1 | 1; tabId: string } | null>(null);
  const jumpCount = useRef(0);
  // What each new tab says until its first output, by tab id.
  const [startingTexts, setStartingTexts] = useState<StartingTexts>({});
  // Every tab the core has listed, to tell a tab closed before it printed from one not listed yet.
  const listed = useRef(new Set<string>());
  const size = useRef(DEFAULT_SIZE);
  const onSize = useCallback((cols: number, lines: number) => {
    if (cols > 0 && lines > 0) size.current = { cols, rows: lines };
  }, []);

  // No sessions (first run, everything closed): the launcher opens. A restarted core seeds the saved tabs, so it opens only if there are none.
  useEffect(() => {
    if (launcherOpens(loaded, rows.length)) setLauncherOpen(true);
  }, [loaded, rows.length]);

  useEffect(() => {
    const current = new Set(rows.map((r) => r.id));
    setStartingTexts((t) => pruneClosed(t, current, listed.current));
    current.forEach((id) => listed.current.add(id));
  }, [rows]);

  // Close, reopen, rename, reorder and go to: see sessionActions. The class reads the latest list through this ref.
  // Only the expanded sidebar has rows to rename (MainWindow shows the rail, or nothing in focus mode, instead).
  const sidebarPref = sidebarPrefOf(view);
  const sidebarExpanded = sidebarMode(width, sidebarPref) === "expanded";
  // The sidebar command says what it will do now: fold the expanded sidebar, or open it again.
  // The grouping command does the same: it names what it does next.
  const paletteLabels = useMemo(() => ({ sidebar: sidebarPref === "expanded" ? "Collapse sidebar to the rail" : "Expand the sidebar", "group-by-project": view.grouped ? "Stop grouping by project" : "Group sessions by project" }), [sidebarPref, view.grouped]);
  // Grouping shows in the expanded sidebar only; the rail and focus layouts keep the core's order, and so do their moves.
  const groupedRows = view.grouped && sidebarExpanded ? rows : undefined;
  const context = useRef({ endpoint, infos, selectedId: focusedId, sidebarExpanded, groupedRows });
  context.current = { endpoint, infos, selectedId: focusedId, sidebarExpanded, groupedRows };
  const pickRef = useRef(pick);
  pickRef.current = pick;
  const actions = useMemo(() => new SessionActions({ api: sessions, context: () => context.current, select: (id) => pickRef.current(id), size: () => size.current }), []);

  // A window of one empty pane takes the store's selection (a first start, or the only open session closed and another opened).
  // With several panes a closed session's pane stays empty.
  useEffect(() => {
    if (settled && loaded && fillsEmptyPane(view.panes, selectedId)) dispatch({ type: "select", tabId: selectedId! });
  }, [settled, loaded, selectedId, view.panes, dispatch]);
  useEffect(() => () => actions.dispose(), [actions]);
  const { toast, closing, renaming, focusToken, resuming, resumed } = useSyncExternalStore(actions.subscribe, actions.getState);
  // The Ended or Exited row each tab had when Resume was pressed, and how many resumes of it had come back ok by then. The core says
  // Running before the session exists (and may say it after the answer), but the strip, and the button that has the focus, stay put
  // until a snapshot shows the tab has really moved on (holdsResume), so it is drawn from this. Presence of an entry means "held".
  const resumeRows = useRef(new Map<string, { row: RowModel; resumed: number }>());
  const startResume = useCallback(
    (row: RowModel) => {
      // One press at a time: while a tab is held its request is out, or answered and not yet seen as Running.
      if (resumeRows.current.has(row.id)) return;
      if (row.status === "ended") resumeRows.current.set(row.id, { row, resumed: actions.getState().resumed[row.id] ?? 0 });
      void actions.resume(row.id);
    },
    [actions],
  );
  // The held row of a tab, or undefined once the hold is over.
  const heldRow = useCallback(
    (live: RowModel): RowModel | undefined => {
      const held = resumeRows.current.get(live.id);
      if (!held) return undefined;
      const response = resuming.includes(live.id) ? "pending" : (resumed[live.id] ?? 0) > held.resumed ? "ok" : "refused";
      return holdsResume(resumeStateOf(held.row), resumeStateOf(live), response) ? held.row : undefined;
    },
    [resuming, resumed],
  );
  // A hold that is over (refused, or the snapshot shows the new state) is forgotten, so the tab can be resumed again later.
  useEffect(() => {
    for (const id of [...resumeRows.current.keys()]) {
      const live = rows.find((r) => r.id === id);
      if (!live || !heldRow(live)) resumeRows.current.delete(id);
    }
  }, [rows, heldRow]);
  const focusedRow = rows.find((r) => r.id === focusedId);
  const resumable = focusedRow?.status === "ended";

  // The core's tasks (worktrees). A session whose folder is a task's is a task: its row says so and offers its review. Asked on start
  // and again after every create, merge and remove; the core has no push for them. Null means it has not loaded (or never answered):
  // that is not "no tasks", so a review asked for then loads it again first (actOn) instead of saying "This session isn't a task."
  const [tasks, setTasks] = useState<Task[] | null>(null);
  const endpointRef = useRef(endpoint);
  endpointRef.current = endpoint;
  const refreshTasks = useMemo(() => taskRefresher(() => (endpointRef.current ? tasksApi.list(endpointRef.current) : Promise.resolve([])), setTasks), []);
  useEffect(() => {
    if (endpoint) void refreshTasks();
  }, [endpoint, refreshTasks]);
  const taskRows = useMemo(() => markTasks(rows, infos, tasks), [rows, infos, tasks]);
  // While the list is unknown the palette keeps Review for the session in focus, so choosing it can load the list again.
  const focusedIsTask = reviewTarget(infos, focusedId, tasks).kind !== "none";
  const hiddenCommands = paletteHidden({ resumable, isTask: focusedIsTask, missing: missingTask(tasks) !== null, focused: infos.some((i) => i.tab_id === focusedId) });

  // The Review sheet. It holds its own snapshot (the task, its changes, the session it was opened for), not a view of the lists above,
  // so a task list or a session list that changes under it, a merge removing the task included, never unmounts it while the core works.
  const [review, setReview] = useState<{ endpoint: Endpoint; tabId: string; task: Task; changes: TaskChanges } | null>(null);
  const reviewRef = useRef(review);
  reviewRef.current = review;
  const opening = useRef(false);
  const openReview = useCallback(
    async (tabId: string, task: Task) => {
      const ep = endpointRef.current;
      if (!ep || opening.current || reviewRef.current) return;
      opening.current = true;
      try {
        setReview({ endpoint: ep, tabId, task, changes: await changesFor(task, () => tasksApi.changes(ep, task.id)) });
      } catch (e) {
        announce(`Couldn't open the review: ${e instanceof ApiError ? e.message : String(e)}`);
      } finally {
        opening.current = false;
      }
    },
    [announce],
  );
  // Review the session in `tabId`, or say it isn't a task. What the state was when the key or button was pressed is read from refs.
  const stateRef = useRef({ infos, tasks, focusedId, launcherOpen, overlay, review });
  stateRef.current = { infos, tasks, focusedId, launcherOpen, overlay, review };
  // Does what a review request came to: opens the sheet, or says why not (it isn't a task, or the task list can't be loaded).
  const actOn = useCallback(
    async (target: ReviewTarget) => {
      const t = await resolveTarget(target, () => stateRef.current.infos, refreshTasks);
      if (t.kind === "open") void openReview(t.tabId, t.task);
      else announce(t.message);
    },
    [openReview, announce, refreshTasks],
  );
  const reviewSession = useCallback(
    (tabId: string | undefined) => {
      const s = stateRef.current;
      void actOn(reviewTarget(s.infos, tabId, s.tasks));
    },
    [actOn],
  );
  const reviewReason = (e: unknown) => (e instanceof ApiError ? e.message : String(e));
  // "Ask <agent> to fix them": the prompt is typed into the task's session, never sent (no Enter), and the terminal gets the focus.
  const askAgent = (text: string) => {
    const r = reviewRef.current;
    if (!r) return;
    sessions.input(r.endpoint, r.tabId, text).then(
      () => {
        pick(r.tabId);
        actions.focusTerminal();
      },
      (e: unknown) => announce(`Couldn't type that into the agent's terminal: ${reviewReason(e)}`),
    );
  };
  // A question about a session that is no longer the selected one, or is gone, has nothing left to ask.
  useEffect(() => {
    if (closing !== null && (closing !== focusedId || !infos.some((i) => i.tab_id === closing))) actions.cancelClose();
  }, [closing, focusedId, infos, actions]);
  useEffect(() => {
    // The session went away, or the window shrank to a layout without the sidebar's rows.
    if (renaming !== null && (!sidebarExpanded || !infos.some((i) => i.tab_id === renaming))) actions.cancelRename({ refocus: false });
  }, [renaming, infos, sidebarExpanded, actions]);
  // Escape cancels the bar from anywhere. Captured, so it never reaches the terminal: Escape would interrupt the agent the bar is about.
  const barOpen = closing !== null;
  useEffect(() => {
    if (!barOpen) return;
    const onEscape = (e: KeyboardEvent) => {
      if (!cancelsCloseBar(e, true)) return;
      e.preventDefault();
      e.stopPropagation();
      actions.cancelClose();
    };
    window.addEventListener("keydown", onEscape, true);
    return () => window.removeEventListener("keydown", onEscape, true);
  }, [barOpen, actions]);

  const selectedRef = useRef(focusedId);
  selectedRef.current = focusedId;
  const jumpPrompt = useCallback((by: -1 | 1) => {
    const tabId = selectedRef.current;
    if (tabId !== undefined) setPromptJump({ token: ++jumpCount.current, by, tabId });
  }, []);
  const onPromptJump = useCallback(
    (by: -1 | 1, result: { index: number; total: number; exit: number | null } | null) => {
      announce(jumpWords(by, result));
    },
    [announce],
  );

  // The shell panel in the sidebar. Its shell is a pty the core keeps out of the tab list. It is asked for the first time the panel is
  // shown, in the folder of the session in focus then, and is never moved after that. Hiding the panel only folds it: the shell keeps
  // running and the same terminal comes back, with no second request. "Open in a pane" turns the shell into a tab; the panel then has
  // no shell, and asks for a fresh one (in whatever folder is focused by then) while it is shown.
  const [panel, setPanel] = useState<{ endpoint: Endpoint; tabId: string; folder: string } | null>(null);
  // The shell the core has turned into a tab, waiting to be listed so a pane can show it.
  const [adopted, setAdopted] = useState<{ endpoint: Endpoint; tabId: string } | null>(null);
  const shellShown = settled && loaded && sidebarExpanded && !view.shell_hidden;
  const focusedCwd = infos.find((i) => i.tab_id === focusedId)?.cwd;
  const focusedCwdRef = useRef(focusedCwd);
  focusedCwdRef.current = focusedCwd;
  const hasFocusedCwd = focusedCwd !== undefined;
  const asking = useRef<Endpoint | null>(null);
  // The endpoint whose shell could not be started: the panel says so, with a Try again button. `attempt` is that button.
  const [failedFor, setFailedFor] = useState<Endpoint | null>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const cwd = focusedCwdRef.current;
    if (!endpoint || !shellShown || cwd === undefined || panel?.endpoint === endpoint || asking.current === endpoint) return;
    const ep = endpoint;
    asking.current = ep;
    setFailedFor(null);
    sessions
      .shellPanel(ep, cwd, size.current)
      .then((tabId) => setPanel({ endpoint: ep, tabId, folder: lastSegment(cwd) }))
      .catch((e: unknown) => {
        // Said once, here: the failure stands until the person asks again (Try again, or hiding and showing the panel).
        console.warn("Could not start the shell panel", e);
        setFailedFor(ep);
        announce(SHELL_FAILED);
      })
      .finally(() => {
        if (asking.current === ep) asking.current = null;
      });
  }, [endpoint, shellShown, hasFocusedCwd, panel, attempt, announce]);
  const panelTab = panel && panel.endpoint === endpoint ? panel : null;
  const shellFailed = failedFor !== null && failedFor === endpoint && !panelTab;
  const retryShell = useCallback(() => {
    setFailedFor(null);
    setAttempt((n) => n + 1);
  }, []);
  // The user asked for the panel while it was folded for lack of room: it opens with less than its usual share (see `shellLayout`).
  const [shellForced, setShellForced] = useState(false);
  // The header button: show it (forcing it open if it had folded itself), or hide it.
  const toggleShell = useCallback(() => {
    const hidden = viewRef.current.shell_hidden;
    const folded = hidden || shellAutoFolded(document);
    // Only a panel folded for lack of room has nothing to toggle in the view: it just opens.
    if (hidden || !folded) dispatch({ type: "toggleShell" });
    setShellForced(folded);
  }, [dispatch]);

  const adopting = useRef(false);
  const openInPane = useCallback(async () => {
    // Nothing to open while the shell is still starting or failed to start.
    if (!endpoint || !panelTab || adopting.current) return;
    adopting.current = true;
    try {
      setAdopted({ endpoint, tabId: await sessions.adoptShellPanel(endpoint) });
    } catch (e) {
      // A 404 means the core has no panel shell (it was adopted already): there is nothing to open.
      console.warn("Could not open the shell in a pane", e);
    } finally {
      adopting.current = false;
    }
  }, [endpoint, panelTab]);
  // The adopted tab is listed: in one update the panel lets go of its terminal and a pane takes the tab. Until then the window draws
  // nothing for that tab (see renderTerminal), so the pty only ever has one terminal attached. (Adopting earlier would let the
  // view's own clean-up drop an id the list does not hold yet.)
  useEffect(() => {
    if (!adopted) return;
    if (adopted.endpoint !== endpoint) return void setAdopted(null);
    const id = adopted.tabId;
    if (!infos.some((i) => i.tab_id === id)) return;
    setAdopted(null);
    setPanel(null);
    const v = viewRef.current;
    if (v.panes.includes(id)) {
      pick(id);
      return;
    }
    // An empty pane on screen, else a new pane if the window has room, else the focused pane (what it held stays in the sidebar).
    const fit = fitRef.current ?? PANE_LIMIT;
    const target = adoptTarget(v, shownRef.current ?? v.panes.length, fit);
    // A zoomed pane would hide a different one.
    if (v.zoomed !== null && target.index !== v.zoomed) dispatch({ type: "zoom" });
    if (target.split) dispatch({ type: "split", fit });
    dispatch({ type: "assign", index: target.index, tabId: id });
    if (!target.split) dispatch({ type: "focusPane", index: target.index });
    selectRef.current(id);
  }, [adopted, endpoint, infos, pick, dispatch]);
  const shellTerminal = useMemo(
    () => (panelTab && endpoint ? <TerminalView key={panelTab.tabId} endpoint={endpoint} tabId={panelTab.tabId} autoFocus={false} theme={themeFor(undefined)} font={terminalFont} /> : null),
    [endpoint, panelTab, themeFor, terminalFont],
  );

  // The app's shortcuts work from anywhere, the terminal included; the terminal lets these keys through (TerminalView).
  const os = endpoint ? osOfPlatform(endpoint.platform) : undefined;

  // Keyboard navigation (hold Ctrl+Shift): the cursor reaches the panes on screen, the session rows and the top bar.
  const split = view.panes.length > 1;
  const numberOf = (id: string | null | undefined) => (id ? (infos.find((i) => i.tab_id === id)?.number ?? null) : null);
  const navPanes = split ? navPanesOf(shownPanes.map(numberOf), view.zoomed) : [numberOf(focusedId)];
  const navNames = useMemo(() => Object.fromEntries(rows.map((r) => [r.number, r.name])), [rows]);
  const applyNav = (effect: NavEffect) => {
    switch (effect.type) {
      case "select-pane":
        if (split) dispatch({ type: "focusPane", index: effect.index });
        actions.focusTerminal();
        return;
      case "bring-into-view": {
        const id = tabIdOfNumber(infos, effect.number);
        if (id === undefined) return;
        if (!split) return void (pick(id), actions.focusTerminal());
        selectRef.current(id);
        dispatch({ type: "assign", index: effect.pane, tabId: id });
        dispatch({ type: "focusPane", index: effect.pane });
        actions.focusTerminal();
        return;
      }
      case "activate":
        if (effect.id === "palette") setOverlay("palette");
        else if (effect.id === "layout") setLayoutMenu(true);
        else if (effect.id === "launcher") setLauncherOpen(true);
        return;
      case "deselect":
        // No terminal selected: the keys stop going to a program until a pane is chosen again.
        if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
        document.querySelector<HTMLElement>(".window__main")?.focus();
        return;
      case "announce":
        announce(effect.text);
        return;
    }
  };
  const nav = useNavigation({
    os: os ?? "windows",
    enabled: os !== undefined && !launcherOpen && overlay === null && review === null,
    panes: navPanes,
    selectedPane: split ? focusIdx : 0,
    names: navNames,
    root: ref,
    apply: applyNav,
  });
  const sendNav = nav.send;

  // The split view's shortcuts and commands. True when it was one of them.
  const paneCommand = useCallback(
    (shortcut: SessionShortcut | null): boolean => {
      const v = viewRef.current;
      switch (shortcut?.kind) {
        case "split":
          if (!roomForPane(v.panes.length, fitRef.current)) {
            announce(NO_PANE_ROOM);
            return true;
          }
          // A zoomed pane would hide the one being added.
          if (v.zoomed !== null) dispatch({ type: "zoom" });
          dispatch({ type: "split", fit: fitRef.current });
          return true;
        case "zoom":
          if (v.zoomed !== null) dispatch({ type: "zoom" });
          else if (v.panes.length > 1 && v.panes[focusIdxRef.current] !== null) {
            // The zoom applies to the active pane, so the one in view is made active first.
            dispatch({ type: "focusPane", index: focusIdxRef.current });
            dispatch({ type: "zoom" });
          }
          return true;
        // The same events the chords send (useNavigation), so the palette's rows and the keys behave the same.
        case "nextPane":
          sendNav([{ type: "tab" }]);
          return true;
        case "focusMode":
          dispatch({ type: "toggleFocus" });
          return true;
        case "sidebar":
          dispatch({ type: "sidebar", pref: railToggle(sidebarPrefOf(v)) });
          return true;
        case "shellPanel":
          if (!context.current.sidebarExpanded) announce(SHELL_PANEL_ELSEWHERE);
          // Hidden, or folded for lack of room: open it (the user asked, so it keeps one row of the list instead of two), and its
          // terminal takes the focus once it is drawn. Shown but elsewhere: focus it. Already in it: nothing to do.
          else {
            const folded = v.shell_hidden || shellAutoFolded(document);
            if (folded || !shellTerminalFocused(document)) {
              if (v.shell_hidden) dispatch({ type: "toggleShell" });
              if (folded) setShellForced(true);
              focusShellTerminal(document);
            }
          }
          return true;
        case "nonePane":
          sendNav([{ type: "select-none" }]);
          return true;
        default:
          return false;
      }
    },
    [dispatch, announce, sendNav],
  );
  // Review (Ctrl+Shift+R) is read apart from the rest because the window owns the key everywhere, the launcher, the overlays and the
  // sheet included: it is always prevented (it is the webview's hard reload), and acted on only when nothing else holds the keys.
  useEffect(() => {
    if (!os) return;
    const onKey = (e: KeyboardEvent) => {
      const s = stateRef.current;
      const result = reviewKeyDown(e, os, { infos: s.infos, focusedId: s.focusedId, tasks: s.tasks, blocked: keyBlocked({ launcherOpen: s.launcherOpen, overlay: s.overlay, review: s.review !== null, inTextBox: isTextBox(e.target as HTMLElement | null) }) });
      if (result === null || result.kind === "ignored") return;
      void actOn(result);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [os, actOn]);

  useEffect(() => {
    // While the sheet is up it has the keys: Esc, Tab and the lists are its own.
    if (!os || launcherOpen || overlay !== null || review !== null) return;
    const onKey = (e: KeyboardEvent) => {
      const shortcut = sessionShortcut(e, os);
      // Review is read above.
      if (!shortcut || shortcut.kind === "review") return;
      // A text box (the rename box, the launcher's fields) keeps its own keys; xterm's own hidden textarea is the terminal, not a box.
      if (isTextBox(e.target as HTMLElement | null)) return;
      e.preventDefault();
      // Holding the key repeats it; only moving a session along the list, or from prompt to prompt, should.
      if (e.repeat && shortcut.kind !== "move" && shortcut.kind !== "prompt") return;
      if (shortcut.kind === "launcher") setLauncherOpen(true);
      else if (shortcut.kind === "palette") setOverlay("palette");
      else if (shortcut.kind === "cheatsheet") setOverlay("shortcuts");
      else if (shortcut.kind === "prompt") jumpPrompt(shortcut.by);
      else if (!paneCommand(shortcut)) actions.handle(shortcut);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [os, launcherOpen, overlay, review, actions, jumpPrompt, paneCommand]);

  // A palette row: a session to go to, the profiles page, or a command the window runs (LivePalette lists only those).
  const runFromPalette = (id: string) => {
    setOverlay(id === "profiles" || id === "appearance" ? id : id === "cheatsheet" ? "shortcuts" : null);
    if (id.startsWith("session:")) return pick(id.slice("session:".length));
    // Palette-only commands. The menu opens after the palette has given the focus back, so it keeps the focus it takes.
    if (id === "unsplit") return dispatch({ type: "remove", index: focusIdxRef.current });
    if (id === "layout") return void setTimeout(() => setLayoutMenu(true), 0);
    if (id === "group-by-project") return dispatch({ type: "toggleGrouped" });
    if (id === "resume") return void (focusedRow && startResume(focusedRow));
    if (id === "copy-recording-path") return void (endpoint && focusedId && copyRecordingPath(endpoint, focusedId, announce));
    // After the palette has given focus back, so the sheet takes it from the right place and returns it there.
    if (id === "review-task") return void setTimeout(() => reviewSession(focusedId), 0);
    // A task whose folder is gone has no session: its sheet (only the remove step) opens for no tab.
    if (id === "clean-up-task") {
      const gone = missingTask(stateRef.current.tasks);
      return void (gone && setTimeout(() => void openReview("", gone), 0));
    }
    const shortcut = commandShortcut(id);
    // Both move the focus, so they wait for the palette to hand it back first.
    if (shortcut?.kind === "nonePane" || shortcut?.kind === "shellPanel") return void setTimeout(() => paneCommand(shortcut), 0);
    if (paneCommand(shortcut)) return;
    if (shortcut?.kind === "launcher") setLauncherOpen(true);
    else if (shortcut?.kind === "prompt") jumpPrompt(shortcut.by);
    // After the palette has given focus back, so a rename box or the terminal keeps the focus it is given.
    else if (shortcut && shortcut.kind !== "palette") setTimeout(() => actions.handle(shortcut), 0);
  };

  const closingInfo = closing === null ? undefined : infos.find((i) => i.tab_id === closing);

  const started = (created: CreateSessionResponse, text: string) => {
    setStartingTexts((t) => ({ ...t, [created.tab_id]: text }));
    pick(created.tab_id);
    setLauncherOpen(false);
    // A task may have been made with it.
    void refreshTasks();
  };

  // One tab's content, in the single window and in every split pane: its terminal, and under it the Resume strip while it is Ended or
  // Exited. An Ended tab has no terminal at all. The wrapper is the same for every state, so a session that exits keeps its terminal
  // (and its scrollback) mounted when the strip appears.
  const renderTerminal = useCallback(
    (row: RowModel, shown = true) => {
      if (!endpoint) return null;
      // The shell panel's terminal still holds this tab's pty (it was just adopted): a pane draws the tab once the panel lets go.
      if (panelTab?.tabId === row.id) return null;
      // While Resume is out the core already says Running, but the strip stays and the terminal waits (see `resuming` in sessionActions).
      const bar = heldRow(row) ?? (row.status === "ended" ? row : undefined);
      const hasTerminal = bar === undefined || bar.phrase !== ENDED_PHRASE;
      const info = infos.find((i) => i.tab_id === row.id);
      const profile = profiles.find((p) => p.id === info?.profile_id);
      const profileGone = info && info.profile_id !== CUSTOM_PROFILE ? resumeDisabledReason(info.profile_id, profiles) : undefined;
      return (
        <div className="resume-pane">
          {hasTerminal && (
            <div className="resume-pane__terminal">
              <TerminalView
                // A resumed tab gets a new terminal, attached after the response (the key moves then, not at Running).
                key={`${row.id}:${resumed[row.id] ?? 0}`}
                endpoint={endpoint}
                tabId={row.id}
                placeholder={startingTexts[row.id]}
                onOutput={() => setStartingTexts((t) => without(t, row.id))}
                onSize={onSize}
                autoFocus={!launcherOpen && overlay === null && review === null && row.id === focusedId}
                visible={shown}
                focusToken={focusToken}
                promptJump={promptJump?.tabId === row.id ? promptJump : undefined}
                onPromptJump={(result) => promptJump && onPromptJump(promptJump.by, result)}
                theme={themeFor(overrideOf(row.id))}
                font={terminalFont}
              />
            </div>
          )}
          {bar && info && (
            <ResumeBar
              row={bar}
              command={resumeCommand(info.agent, profile?.name ?? info.profile_id)}
              folder={info.cwd}
              disabledReason={profileGone}
              onResume={() => startResume(bar)}
            />
          )}
        </div>
      );
    },
    [endpoint, panelTab?.tabId, heldRow, resumed, infos, profiles, startResume, startingTexts, onSize, launcherOpen, overlay, review, focusedId, focusToken, promptJump, onPromptJump, themeFor, overrideOf, terminalFont],
  );

  if (error) {
    return (
      <p className="startup-error" role="alert">
        {error}
      </p>
    );
  }
  return (
    <div ref={ref} className="live-app">
      {endpoint && settled && loaded && (
        <MainWindow
          width={width}
          height={height}
          layout={view.panes.length > 1 ? "split" : "default"}
          sidebarPref={sidebarPref}
          sidebarWidth={view.sidebar_width}
          onSidebarWidth={(w) => dispatch({ type: "sidebarWidth", width: w })}
          onSidebarReset={() => dispatch({ type: "resetSidebarWidth" })}
          paneIds={view.panes}
          activePane={view.active}
          zoomedPane={view.zoomed ?? undefined}
          arrangement={(view.arrangement ?? undefined) as Arrangement | undefined}
          ratios={ratiosOf(view)}
          layoutMenu={layoutMenu}
          onLayoutMenu={setLayoutMenu}
          onVisible={setShownCount}
          onFit={setFitCount}
          onLayout={(count, arrangement) => {
            if (count !== viewRef.current.panes.length) dispatch({ type: "count", count });
            dispatch({ type: "arrange", arrangement });
          }}
          onPane={dispatch}
          rows={taskRows}
          selectedId={focusedId}
          requests={[]}
          ports={[]}
          footer={null}
          shell={{ terminal: shellTerminal, cwd: panelTab?.folder, hidden: view.shell_hidden, failed: shellFailed, onRetry: retryShell, openDisabled: !panelTab, forceOpen: shellForced, onToggle: toggleShell, onOpenInPane: () => void openInPane() }}
          composer={false}
          statusBar={false}
          renderTerminal={renderTerminal}
          headerBar={closingInfo && <CloseBar text={closeBarText(closingInfo)} onCancel={actions.cancelClose.bind(actions)} onCloseNow={() => void actions.confirmClose()} />}
          toast={toast && <Toast text={toast.text} actionLabel={toast.reopenId ? "Reopen" : undefined} onAction={() => void actions.reopenFromToast()} />}
          notice={toast ? { id: toast.id, text: toast.text } : notice}
          renamingId={renaming}
          onRename={(id, name) => void actions.commitRename(id, name)}
          onRenameCancel={(opts) => actions.cancelRename(opts)}
          onReorder={(ids) => actions.reorder(ids)}
          grouped={view.grouped}
          onSelect={pick}
          onReview={reviewSession}
          onPalette={() => setOverlay("palette")}
          noneSelected={nav.noneSelected}
          navCursor={nav.cursor}
        />
      )}
      {overlay === "palette" && os && <LivePalette sessions={paletteSessions(rows)} os={os} labels={paletteLabels} hidden={hiddenCommands} onRun={runFromPalette} onClose={() => setOverlay(null)} />}
      {review && (
        <ReviewSheet
          task={review.task}
          changes={review.changes}
          agentName={reviewAgentName(infos.find((i) => i.tab_id === review.tabId))}
          api={reviewApi(review.endpoint, review.task.id)}
          onClose={() => setReview(null)}
          onDone={(message) => {
            actions.notify(message);
            void refreshTasks();
          }}
          onAskAgent={askAgent}
        />
      )}
      {overlay === "profiles" && endpoint && <ProfilesDialog endpoint={endpoint} onClose={() => setOverlay(null)} onNotice={announce} onSaved={reloadProfiles} />}
      {overlay === "appearance" && <AppearanceDialog onClose={() => setOverlay(null)} onNotice={announce} />}
      {overlay === "shortcuts" && os && <LiveShortcuts os={os} onClose={() => setOverlay(null)} />}
      {launcherOpen && (
        <LiveLauncher
          endpoint={endpoint}
          nextNumber={nextSessionNumber(infos.map((i) => i.number))}
          size={() => size.current}
          onStarted={started}
          onClose={() => setLauncherOpen(false)}
        />
      )}
    </div>
  );
}
