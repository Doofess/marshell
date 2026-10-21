import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import { ApiError } from "../../lib/api/client";
import { sessions } from "../../lib/api/sessions";
import { decide, release } from "../../lib/api/permissions";
import { peek as peekScreen } from "../../lib/api/peek";
import { decisionFor, laneEntries } from "../approval/fromCore";
import { LiveCard, useCards, type Answer } from "../approval/LiveCard";
import type { ApprovalRequest } from "../approval/types";
import { close as closeWindow, minimize as minimizeWindow, onMaximizedChange, toggleMaximize } from "../../lib/window/appWindow";
import { copyRecordingPath } from "./recording";
import { addFailure, clipboardText, deliveryFor, noticeFor } from "../regions/deliver";
import { regionText } from "../regions/region";
import { pasteAttachment, pastePaths, saveAttachment } from "../regions/regionsApi";
import { toPng } from "../regions/imagePaste";
import { CAPTURE_CANCELLED_NOTICE, CAPTURE_WAIT_NOTICE, cancelCapture, captureFailure, captureRefusal, captureScreenRegion, readClipboardImage, watchFocusCancel } from "../capture/capture";
import { deliveryTo, type DropTarget } from "../drop/dropTarget";
import { useFileDrop } from "../drop/useFileDrop";
import { statFile } from "../preview/previewApi";
import { inMenu } from "../../components/ContextMenu/menuKeys";
import { focusAfterMenu, menuFacts, modeOfItem } from "./sessionMenuRun";
import { ContextMenu } from "../../components/ContextMenu/ContextMenu";
import { tasksApi } from "../../lib/api/tasks";
import { cancelsCloseBar, commandShortcut, sessionShortcut, type SessionShortcut } from "../../lib/appShortcuts";
import { nextWaiting } from "../lane/laneLayout";
import { osOfPlatform } from "../../lib/keymap";
import { paletteSessions } from "../palette/sessions";
import { Preview } from "../preview/Preview";
import { PreviewContext } from "../preview/PreviewContext";
import { currentOf, usePreview } from "../preview/previewState";
import { ProfilesDialog } from "../settings/ProfilesPage";
import { modeMenuItems, sessionMenuItems } from "../sidebar/sessionMenu";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import type { Mode, RowModel } from "../sidebar/types";
import { AgentsDialog } from "../agents/AgentsDialog";
import { DiagnosticsDialog } from "../diagnostics/DiagnosticsDialog";
import { LiveOnboarding } from "../onboarding/LiveOnboarding";
import { getSetup } from "../../lib/api/agents";
import type { HookInstall } from "../../generated/HookInstall";
import { AppearanceDialog } from "../appearance/AppearanceDialog";
import { useAppearance } from "../appearance/AppearanceProvider";
import { listen } from "@tauri-apps/api/event";
import { needsYouCount } from "../attention/attention";
import { useAttention } from "../attention/useAttention";
import { notify } from "../../lib/api/notify";
import { traySessions, useTray, useTrayActions, type TrayAction } from "../tray/useTray";
import { CloseDialog } from "../window/CloseDialog";
import { NotificationsDialog } from "../notifications/NotificationsDialog";
import { useViewReport } from "../notifications/useViewReport";
import { terminalThemeOf, wtFont } from "../appearance/terminalSettings";
import { changesFor, isTextBox, keyBlocked, markTasks, missingTask, paletteHidden, resolveTarget, reviewAgentName, reviewKeyDown, reviewTarget, taskRefresher, type ReviewTarget } from "../tasks/reviewOpen";
import { ReviewSheet } from "../tasks/ReviewSheet";
import { reviewApi } from "../tasks/reviewRun";
import { jumpWords } from "../terminal/promptMarks";
import { resolveTerminalFont } from "../terminal/terminalFont";
import { TerminalView, type TerminalThemeProp } from "../terminal/TerminalView";
import { appScheme, resolveTerminalTheme } from "../terminal/terminalTheme";
import type { Notice } from "../window/announce";
import { sidebarMode } from "../window/layout";
import { MainWindow } from "../window/MainWindow";
import { PANE_LIMIT, type Arrangement } from "../window/splitLayout";
import { useViewState } from "../window/useViewState";
import { adoptTarget, fillsEmptyPane, focusedIndex, paneToggle, ratiosOf, roomForPane, sidebarPrefOf, sidebarToggleFor, viewReducer, visibleSessions, type ViewState } from "../window/viewState";
import { CloseBar } from "./CloseBar";
import { PauseBar } from "./PauseBar";
import { RestartBar } from "./RestartBar";
import { launcherOpens, type OnboardingShown } from "./launcherOpens";
import { LiveLauncher } from "./LiveLauncher";
import { LivePalette } from "./LivePalette";
import { LiveShortcuts } from "./LiveShortcuts";
import { ResumeBar } from "./ResumeBar";
import { sessionResumeCommand } from "./resumeCommand";
import { holdsResume, offersModeChange, offersResume, resumeBlockedReason, resumeStateOf } from "./resumeState";
import { PAUSED_TYPING_TEXT, SessionActions, closeBarText, needsCloseConfirm, pauseBarStale, pauseBarText, restartBarStale, restartBarText, tabIdOfNumber } from "./sessionActions";
import { navPanesOf, useNavigation } from "./useNavigation";
import type { NavEffect } from "../../lib/navigation/types";
import { Toast } from "./Toast";
import { pruneClosed, without, type StartingTexts } from "./startingTexts";
import { SHELL_FAILED } from "../window/ShellPanel";
import { handedOff } from "./handOff";
import { focusShellTerminal, shellAutoFolded, shellTerminalFocused } from "./shellPanelFocus";
import { ENDED_PHRASE, lastSegment, planRing } from "./toRow";
import { useSessions } from "./useSessions";

/** The size a new session starts at until a terminal has been measured. */
const DEFAULT_SIZE = { cols: 120, rows: 30 };
/** What the announcer says when the sidebar is asked to open in a window under 960 px, where it cannot. */
const SIDEBAR_NEEDS_WIDTH = "Widen the window to open the sidebar.";
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
  // Claude Code's hook state, read when the window opens and again after each change in the Agents dialog or the onboarding. Never polled.
  const [claudeHooks, setClaudeHooks] = useState<HookInstall | undefined>(undefined);
  const { endpoint, profiles, rows, infos, permissions, now, selectedId, loaded, select, reloadProfiles, error } = useSessions(useMemo(() => ({ claude: claudeHooks }), [claudeHooks]));
  const refreshSetup = useCallback(() => {
    if (!endpoint) return;
    getSetup(endpoint, "claude").then(
      (s) => setClaudeHooks(s.found ? s.hooks : undefined),
      () => {},
    );
  }, [endpoint]);
  useEffect(refreshSetup, [refreshSetup]);
  const [maximized, setMaximized] = useState(false);
  useEffect(() => onMaximizedChange(setMaximized), []);
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
  // The preview pane: its history lives here so it outlasts a session switch. Open, it is about the focused session's window only.
  const preview = usePreview();
  const previewRef = useRef(preview);
  previewRef.current = preview;
  const previewEntry = preview.state.open ? currentOf(preview.state) : undefined;
  // Every session on screen, which the sidebar and the rail mark.
  const visibleIds = useMemo(() => visibleSessions(view.panes, shownCount ?? view.panes.length, view.zoomed), [view.panes, shownCount, view.zoomed]);
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
  const { appearance, wtFiles, sync } = useAppearance();
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
  const [overlay, setOverlay] = useState<"palette" | "profiles" | "appearance" | "agents" | "notifications" | "diagnostics" | "shortcuts" | null>(null);
  // The Agents dialog opened from a row's "set up" starts on the Set up card.
  const [agentsFrom, setAgentsFrom] = useState<"row" | null>(null);
  // First run: shown once, when the saved settings say the onboarding was not seen. Nothing shows until that read has answered;
  // a failed read shows none.
  const [onboardingShown, setOnboardingShown] = useState<OnboardingShown>("unknown");
  const onboarding = onboardingShown === "showing";
  useEffect(() => {
    if (!endpoint || !sync) return;
    let live = true;
    sync.load().then(
      () => live && setOnboardingShown(sync.onboarded ? "done" : "showing"),
      () => live && setOnboardingShown("done"),
    );
    return () => {
      live = false;
    };
  }, [endpoint, sync]);
  // "Open file…" opens the palette with files only; any other way of opening it lists everything.
  const [filesOnly, setFilesOnly] = useState(false);
  useEffect(() => {
    if (overlay !== "palette") setFilesOnly(false);
  }, [overlay]);
  // What the profiles page and the prompt jumps say aloud through the window's announcer. Negative ids never meet a toast's.
  const [notice, setNotice] = useState<Notice | null>(null);
  const noticeCount = useRef(0);
  const announce = useCallback((text: string, navigation?: boolean) => setNotice({ id: -++noticeCount.current, text, navigation }), []);
  // The one-time close dialog, opened by the shell's close-choice (see below).
  const [closeAsk, setCloseAsk] = useState<{ busy: boolean; error: string | null } | null>(null);
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

  // No sessions (first run, everything closed): the launcher opens, after the onboarding if one shows. A restarted core seeds the
  // saved tabs, so it opens only if there are none.
  useEffect(() => {
    if (launcherOpens(loaded, rows.length, onboardingShown)) setLauncherOpen(true);
  }, [loaded, rows.length, onboardingShown]);

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
  // A clicked banner brings the window forward (the shell does that) and names the session; no tab means there is nothing to go to.
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    listen<{ tab_id: string | null }>("focus-session", (e) => {
      if (e.payload.tab_id) pickRef.current(e.payload.tab_id);
    })
      .then((s) => (gone ? s() : (stop = s)))
      .catch(() => {}); // Outside Tauri (tests, a plain browser) there are no events.
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
  // The sidebar command and the top bar's sidebar button: fold to the rail or open, or say why a narrow window cannot open it.
  const toggleSidebar = useCallback(
    (v: ViewState) => {
      const pref = sidebarPrefOf(v);
      const next = sidebarToggleFor(pref, context.current.sidebarExpanded ? "expanded" : pref === "expanded" ? "rail" : pref);
      if (next === null) announce(SIDEBAR_NEEDS_WIDTH);
      else dispatch({ type: "sidebar", pref: next });
    },
    [announce, dispatch],
  );
  // Everything that needs the person, oldest first: the held requests and the sessions waiting in their own terminal. This list is the
  // count (lane heading, taskbar badge, announcer). The cards add the receipts and exits that outlast an entry.
  const [sendingRequests, setSendingRequests] = useState<Map<string, string>>(() => new Map());
  const entries = useMemo(() => laneEntries(permissions, rows, Math.max(now, Date.now()), new Set(sendingRequests.values())), [permissions, rows, now, sendingRequests]);
  const cards = useCards(entries, announce);
  const requests = cards.requests;
  // One count for the taskbar badge, the window title and the tray.
  const needsYou = useMemo(() => needsYouCount(entries), [entries]);
  useAttention(needsYou);
  const [drawerId, setDrawerId] = useState<string | undefined>(undefined);
  const drawerRequest = requests.find((r) => r.id === drawerId && r.state.phase === "pending");
  const actions = useMemo(() => new SessionActions({ api: sessions, context: () => context.current, select: (id) => pickRef.current(id), size: () => size.current, settings: sync ?? undefined }), []);

  const answerRequest = useCallback(
    (r: ApprovalRequest, a: Answer) => {
      if (!endpoint) return;
      const verdict = a === "deny" ? "denied" : a === "terminal" ? "terminal" : "allowed";
      cards.answer(r.id, verdict, async () => {
        setSendingRequests((current) => new Map(current).set(r.id, r.tab));
        try {
          return await (a === "terminal" ? release(endpoint, r.id) : decide(endpoint, r.id, decisionFor(a)));
        } finally {
          setSendingRequests((current) => {
            const next = new Map(current);
            next.delete(r.id);
            return next;
          });
        }
      });
    },
    [endpoint, cards.answer],
  );
  const jumpToRequest = useCallback(
    (r: ApprovalRequest) => {
      pick(r.tab);
      setTimeout(() => actions.focusTerminal(), 0);
    },
    [pick, actions],
  );
  const requestsRef = useRef(requests);
  requestsRef.current = requests;
  const jumpToRequestRef = useRef(jumpToRequest);
  jumpToRequestRef.current = jumpToRequest;
  const restoreFocus = useCallback(() => void setTimeout(() => actions.focusTerminal(), 0), [actions]);
  const liveCard = (r: ApprovalRequest, inDrawer = false) => (
    <LiveCard key={r.id + (inDrawer ? "-drawer" : "")} request={r} inDrawer={inDrawer} onAnswer={answerRequest} onJump={jumpToRequest} onExpand={(x) => setDrawerId(x.id)} onRestoreFocus={restoreFocus} />
  );

  // A window of one empty pane takes the store's selection (a first start, or the only open session closed and another opened).
  // With several panes a closed session's pane stays empty.
  useEffect(() => {
    if (settled && loaded && fillsEmptyPane(view.panes, selectedId)) dispatch({ type: "select", tabId: selectedId! });
  }, [settled, loaded, selectedId, view.panes, dispatch]);
  useEffect(() => () => actions.dispose(), [actions]);
  const { toast, closing, restarting, pauseBar, renaming, focusToken, resuming, resumed } = useSyncExternalStore(actions.subscribe, actions.getState);
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
  const focusedInfo = infos.find((i) => i.tab_id === focusedId);
  const resumable = focusedRow !== undefined && offersResume(focusedRow, focusedInfo);

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
  // The footer shows the plan only when a status line has reported it; there is no doctor count to show yet.
  // A shown use goes when its 5-hour window resets, even if no session says anything new by then.
  const [planNow, setPlanNow] = useState(() => Date.now());
  const plan = useMemo(() => planRing(infos, Math.max(planNow, Date.now())), [infos, planNow]);
  useEffect(() => {
    if (plan?.until === undefined) return;
    const id = setTimeout(() => setPlanNow(Date.now()), Math.max(0, plan.until - Date.now()) + 1000);
    return () => clearTimeout(id);
  }, [plan?.until]);
  const footer = useMemo(() => (plan ? { planPct: plan.pct, fiveHourPct: plan.pct } : null), [plan]);
  const taskRows = useMemo(() => markTasks(rows, infos, tasks), [rows, infos, tasks]);
  // While the list is unknown the palette keeps Review for the session in focus, so choosing it can load the list again.
  const focusedIsTask = reviewTarget(infos, focusedId, tasks).kind !== "none";
  const hiddenCommands = paletteHidden({
    resumable,
    isTask: focusedIsTask,
    missing: missingTask(tasks) !== null,
    focused: focusedInfo !== undefined,
    agent: focusedInfo?.agent != null && (focusedRow === undefined || offersModeChange(focusedRow, focusedInfo)),
    pausable: focusedInfo?.state.kind === "running" && !focusedInfo.paused,
    paused: focusedInfo?.state.kind === "running" && focusedInfo.paused,
  });

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
  const stateRef = useRef({ infos, tasks, focusedId, launcherOpen, overlay, review, rows });
  stateRef.current = { infos, tasks, focusedId, launcherOpen, overlay, review, rows };
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
  // "Add" on a marked region: saves the crop, then pastes its reference and comment into the session focused right now (an agent), or
  // puts them on the clipboard (a plain terminal). Never Enter. The terminal gets the focus once the popover has unmounted.
  const addRegion = async (r: { png: Blob; label: string; comment: string }): Promise<void> => {
    const ep = endpointRef.current;
    const { focusedId: id, rows: current } = stateRef.current;
    const target = ep && id ? deliveryFor(current.find((x) => x.id === id) ?? null) : null;
    if (!ep || !id || !target) throw new Error("No session to add it to.");
    if ("refused" in target) throw new Error(target.refused);
    const text = regionText(r.label, r.comment);
    try {
      const saved = await saveAttachment(ep, id, r.png);
      if (target.via === "paste") await pasteAttachment(ep, id, { attachment: saved.name, text });
      else await navigator.clipboard.writeText(clipboardText(saved.reference, text));
    } catch (e) {
      const message = `Couldn't add the region: ${addFailure(e)}`;
      announce(message);
      throw new Error(message);
    }
    announce(noticeFor(target.via, target.name));
    setTimeout(() => actions.focusTerminal(), 0);
  };
  // Files dropped from the system: on a terminal each path is pasted into that session (never Enter); anywhere else the first file opens in
  // the preview. A paused or an ended session is refused before anything is pasted.
  const dropPaths = async (target: DropTarget, paths: string[]): Promise<void> => {
    if (target.kind === "preview") {
      for (const path of paths) {
        const stat = await statFile(path).catch(() => null);
        if (stat?.kind !== "ok") continue;
        preview.open({ path });
        announce(`Opened ${lastSegment(path)} in the preview.`);
        return;
      }
      announce("A folder can't be shown in the preview.");
      return;
    }
    const ep = endpointRef.current;
    const delivery = ep ? deliveryTo(target.tabId, stateRef.current.rows, panelIdRef.current) : null;
    if (!ep || !delivery) return announce("No session to add it to.");
    if (delivery.refused !== undefined) return announce(delivery.refused);
    try {
      await pastePaths(ep, target.tabId, paths);
    } catch (e) {
      return announce(`Couldn't paste the path: ${addFailure(e)}`);
    }
    announce(paths.length === 1 ? `Pasted ${lastSegment(paths[0] ?? "")} into ${delivery.name}.` : `Pasted ${paths.length} paths into ${delivery.name}.`);
    focusDelivered(target.tabId, delivery);
  };
  // The shell panel's tab has no row to select, and focusTerminal would move the keys to the selected session, so the panel's own terminal is focused instead.
  const panelIdRef = useRef<string | undefined>(undefined);
  const focusDelivered = (tabId: string, delivery: { name: string; panel?: true }) => {
    if (delivery.panel) return focusShellTerminal(document);
    pick(tabId);
    setTimeout(() => actions.focusTerminal(), 0);
  };
  // An image pasted into a terminal (Ctrl+Shift+V): saved, and its reference pasted into that session, agent or shell. Never Enter.
  const addImage = async (tabId: string, image: Blob): Promise<void> => {
    const ep = endpointRef.current;
    const delivery = ep ? deliveryTo(tabId, stateRef.current.rows, panelIdRef.current) : null;
    if (!ep || !delivery) return announce("No session to add it to.");
    if (delivery.refused !== undefined) return announce(delivery.refused);
    try {
      const saved = await saveAttachment(ep, tabId, await toPng(image));
      await pasteAttachment(ep, tabId, { attachment: saved.name, text: "" });
    } catch (e) {
      return announce(`Couldn't add the image: ${addFailure(e)}`);
    }
    announce(`Added the image to ${delivery.name}.`);
    focusDelivered(tabId, delivery);
  };
  // Capture screen region: the system's snipping tool, then the image it left on the clipboard goes to the focused session through
  // addImage. The focus watcher runs only while a capture does; a second press while one runs hears the core's "already running"
  // sentence and starts no second watcher. Every way out ends in a notice, and a text copy during the wait is never the capture (the core).
  const stopWatch = useRef<(() => void) | null>(null);
  const runCapture = async (): Promise<void> => {
    const tabId = stateRef.current.focusedId;
    const refused = captureRefusal(tabId, stateRef.current.rows);
    if (refused !== null || !tabId) return announce(refused ?? "No session to add it to.");
    const watching = stopWatch.current !== null;
    if (!watching) {
      announce(CAPTURE_WAIT_NOTICE);
      stopWatch.current = watchFocusCancel(() => void cancelCapture().catch(() => undefined));
    }
    let arrived: boolean;
    try {
      arrived = await captureScreenRegion();
    } catch (e) {
      return announce(captureFailure(e));
    } finally {
      if (!watching) {
        stopWatch.current?.();
        stopWatch.current = null;
      }
    }
    if (!arrived) return announce(CAPTURE_CANCELLED_NOTICE);
    let image: Blob | null;
    try {
      image = await readClipboardImage();
    } catch (e) {
      return announce(`Couldn't add the image: ${captureFailure(e)}`);
    }
    if (!image) return announce("Couldn't add the image: the clipboard has no image.");
    await addImage(tabId, image);
  };
  const runCaptureRef = useRef(runCapture);
  runCaptureRef.current = runCapture;
  useEffect(() => () => stopWatch.current?.(), []);
  // The pane under a dragged file gets the dashed outline and its label (Drop.css).
  const dropOver = useRef<Element | null>(null);
  const dropHover = (target: DropTarget | null) => {
    let next: Element | null = null;
    let label = "";
    if (target?.kind === "terminal") {
      const host = Array.from(document.querySelectorAll("[data-drop-tab]")).find((h) => h.getAttribute("data-drop-tab") === target.tabId);
      const name = deliveryTo(target.tabId, stateRef.current.rows, panelIdRef.current)?.name;
      next = host?.closest(".pane") ?? host ?? null;
      label = `Drop to paste the path into ${name ?? "the session"}`;
    } else if (target) {
      next = document.querySelector("[data-drop-preview]");
      label = "Drop to open in the preview";
    }
    if (dropOver.current && dropOver.current !== next) {
      dropOver.current.removeAttribute("data-drop-over");
      dropOver.current.removeAttribute("data-drop-label");
    }
    next?.setAttribute("data-drop-over", "");
    next?.setAttribute("data-drop-label", label);
    dropOver.current = next;
  };
  useFileDrop({ onPaths: (t, p) => void dropPaths(t, p), onHover: dropHover });
  // A question about a session that is no longer the selected one, or is gone, has nothing left to ask.
  useEffect(() => {
    if (closing !== null && (closing !== focusedId || !infos.some((i) => i.tab_id === closing))) actions.cancelClose();
  }, [closing, focusedId, infos, actions]);
  // The same for the restart bar and the Pause bar (they share the pane's bar slot); a close question and either of them are never
  // up together. A session that askRestart or pauseSession is bringing on screen is not focused yet for a render or two (a split
  // waits for the pane count); that is not a move away, as long as the focus has not moved since the question.
  const showingForRestart = useRef<{ id: string; from: string | undefined } | null>(null);
  const slotId = restarting?.id ?? pauseBar?.id ?? null;
  const slotSession = infos.find((i) => i.tab_id === slotId);
  useEffect(() => {
    const pending = showingForRestart.current;
    if (slotId === null || pending?.id !== slotId || focusedId === slotId) showingForRestart.current = null;
    if (slotId === null) return;
    const showing = pending?.id === slotId && focusedId === pending.from ? pending.id : null;
    const ids = infos.map((i) => i.tab_id);
    if (restarting !== null) {
      if (restartBarStale({ id: slotId, closing, focusedId, ids, showing })) actions.cancelRestart();
    } else if (pauseBar !== null && pauseBarStale({ id: slotId, closing, focusedId, ids, showing, kind: pauseBar.kind, paused: slotSession?.paused === true })) {
      actions.cancelPause();
    }
  }, [slotId, restarting, pauseBar, slotSession?.paused, closing, focusedId, infos, actions]);
  useEffect(() => {
    // The session went away, or the window shrank to a layout without the sidebar's rows.
    if (renaming !== null && (!sidebarExpanded || !infos.some((i) => i.tab_id === renaming))) actions.cancelRename({ refocus: false });
  }, [renaming, infos, sidebarExpanded, actions]);
  // Escape cancels the bar from anywhere. Captured, so it never reaches the terminal: Escape would interrupt the agent the bar is about.
  const barOpen = closing !== null || restarting !== null || pauseBar !== null;
  useEffect(() => {
    if (!barOpen) return;
    const onEscape = (e: KeyboardEvent) => {
      // An open row menu has its own Escape, which closes it and nothing else.
      if (!cancelsCloseBar(e, true) || inMenu(e.target)) return;
      e.preventDefault();
      e.stopPropagation();
      actions.cancelClose();
      actions.cancelRestart();
      actions.cancelPause();
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
  panelIdRef.current = panelTab?.tabId;
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
  // Shows a session in a pane: where it already is, else an empty pane on screen, else a new pane if the window has room, else the
  // focused pane (what it held stays in the sidebar). The adopted shell panel and the row menu's "Split into a pane" both use it.
  const showInPane = useCallback(
    (id: string) => {
      const v = viewRef.current;
      if (v.panes.includes(id)) {
        pick(id);
        return;
      }
      const fit = fitRef.current ?? PANE_LIMIT;
      const target = adoptTarget(v, shownRef.current ?? v.panes.length, fit);
      // A zoomed pane would hide a different one.
      if (v.zoomed !== null && target.index !== v.zoomed) dispatch({ type: "zoom" });
      if (target.split) dispatch({ type: "split", fit });
      dispatch({ type: "assign", index: target.index, tabId: id });
      if (!target.split) dispatch({ type: "focusPane", index: target.index });
      selectRef.current(id);
    },
    [pick, dispatch],
  );
  // Ctrl+click or Ctrl+Space on a session: drop its pane if it is on screen (never the last one), else show it as Split does.
  const togglePane = useCallback(
    (id: string) => {
      const v = viewRef.current;
      const toggle = paneToggle(v, id, shownRef.current ?? v.panes.length, fitRef.current ?? PANE_LIMIT);
      if (toggle.kind === "remove") {
        const action = { type: "remove", index: toggle.index } as const;
        dispatch(action);
        // The store's selection follows the pane that has the focus now, so it never names the session whose pane went.
        const after = viewReducer(v, action);
        const next = after.panes[focusedIndex(after.panes.slice(0, shownRef.current ?? after.panes.length), after.active, after.zoomed)];
        if (next) selectRef.current(next);
      } else if (toggle.kind === "show") showInPane(id);
      else if (toggle.kind === "full") announce(NO_PANE_ROOM);
    },
    [dispatch, showInPane, announce],
  );
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
    showInPane(id);
  }, [adopted, endpoint, infos, showInPane]);
  // The core adopted the panel's shell because it runs a known agent: its tab is listed, so treat it like "Open in a pane".
  // Idempotent: `handedOff` is null once the adopt is pending, and the effect above clears it when done.
  const handed = handedOff(panelTab?.tabId, adopted?.tabId, infos);
  useEffect(() => {
    if (handed && endpoint) setAdopted({ endpoint, tabId: handed });
  }, [handed, endpoint]);
  const shellTerminal = useMemo(
    () => (panelTab && endpoint ? <TerminalView key={panelTab.tabId} endpoint={endpoint} tabId={panelTab.tabId} autoFocus={false} theme={themeFor(undefined)} font={terminalFont} onImagePaste={(image) => void addImage(panelTab.tabId, image)} /> : null),
    [endpoint, panelTab, themeFor, terminalFont],
  );

  // The app's shortcuts work from anywhere, the terminal included; the terminal lets these keys through (TerminalView).
  const os = endpoint ? osOfPlatform(endpoint.platform) : undefined;
  // The row menu: which items a row shows, and what each one does. Every action takes the row's own id, so it never waits for a
  // selection update to land.
  const menuItems = useCallback(
    (id: string) => sessionMenuItems(menuFacts(id, { infos, profiles, rows: taskRows, panes: view.panes, shown: shownRef.current ?? view.panes.length, fit: fitRef.current ?? PANE_LIMIT }), os ?? "windows"),
    [infos, profiles, taskRows, view.panes, os],
  );
  // Where focus goes when the mode menu ends without a bar: back to the row or the chip it came from.
  const returnFromModeMenu = (m: { id: string; from: "row" | "chip" }) => {
    if (m.from === "chip") return void document.querySelector<HTMLElement>(".session-header__mode")?.focus();
    document.querySelector<HTMLElement>(`[data-nav-row="${rows.find((r) => r.id === m.id)?.number}"]`)?.focus();
  };
  // Mode ▸: the second menu (the row menu's item, the header chip), and the palette's Mode commands, all end in `askRestart`.
  const [modeMenu, setModeMenu] = useState<{ id: string; anchor: { x: number; y: number }; from: "row" | "chip" } | null>(null);
  useEffect(() => {
    if (!modeMenu) return;
    const info = infos.find((i) => i.tab_id === modeMenu.id);
    const row = rows.find((r) => r.id === modeMenu.id);
    if (!info) setModeMenu(null);
    else if (row && !offersModeChange(row, info)) {
      setModeMenu(null);
      actions.focusTerminal();
    }
  }, [modeMenu, infos, rows, actions]);
  // Asks "Restart … in <mode>?" for a session. The bar is on the focused session's pane, so a session that is off screen or not
  // focused is shown first, as a plain click would. False (a plain shell, or the mode it is already in) changes nothing.
  const askRestart = useCallback(
    (id: string, mode: Mode): boolean => {
      const row = rows.find((r) => r.id === id);
      if (row && !offersModeChange(row, infos.find((i) => i.tab_id === id))) return false;
      if (!actions.askRestart(id, mode)) return false;
      actions.cancelClose();
      if (!(shownPanes.includes(id) && id === focusedId)) {
        showingForRestart.current = { id, from: focusedId };
        showInPane(id);
      }
      return true;
    },
    [actions, shownPanes, focusedId, showInPane, rows, infos],
  );
  // Pause: the first one ever asks first, on the session's pane like the restart question; after that it acts at once.
  const pauseSession = useCallback(
    (id: string): "asked" | "sent" | null => {
      const r = actions.pause(id);
      if (r === "asked" && !(shownPanes.includes(id) && id === focusedId)) {
        showingForRestart.current = { id, from: focusedId };
        showInPane(id);
      }
      return r;
    },
    [actions, shownPanes, focusedId, showInPane],
  );
  const onMenuRun = useCallback(
    (id: string, itemId: string, anchor: { x: number; y: number }) => {
      const info = infos.find((i) => i.tab_id === id);
      switch (itemId) {
        // The menu that ran this is gone and the mode menu takes its place at the same spot, with its own focus.
        case "mode":
          setModeMenu({ id, anchor, from: "row" });
          break;
        // No timeout: the rename box mounts in the same update that closes the menu, and the menu closes without handing focus back.
        case "rename":
          actions.startRename(id);
          break;
        case "split":
          showInPane(id);
          break;
        case "resume": {
          const row = rows.find((r) => r.id === id);
          if (row) startResume(row);
          break;
        }
        case "pause":
          if (pauseSession(id) === "sent") actions.focusTerminal();
          break;
        case "continue":
          void actions.cont(id);
          break;
        case "mute":
        case "unmute":
          if (endpoint) void sessions.setMuted(endpoint, id, itemId === "mute").catch(() => announce("Couldn't change the mute. Try again."));
          break;
        // After the menu has gone, so the sheet takes the focus and the menu does not give it back to the row.
        case "review-task":
          setTimeout(() => reviewSession(id), 0);
          break;
        case "close": {
          // The close bar belongs to the focused session (it goes away when focus moves off it), so a session that needs the bar is focused first.
          if (info && needsCloseConfirm(info)) pick(id);
          void actions.requestClose(id);
          break;
        }
      }
      // The menu item that had focus is gone with the menu: put focus somewhere real instead of the page.
      const to = focusAfterMenu(itemId, info !== undefined && needsCloseConfirm(info));
      if (to === "terminal") actions.focusTerminal();
      else if (to === "row") document.querySelector<HTMLElement>(`[data-nav-row="${rows.find((r) => r.id === id)?.number}"]`)?.focus();
    },
    [actions, showInPane, rows, infos, pick, startResume, reviewSession, pauseSession, endpoint, announce],
  );

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
        // The cursor's receipt speaks for a switch it causes, so the focus-moved line is not said as well.
        announce(effect.text, true);
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
        // With nothing opened yet there is nothing to bring back: the palette is where a file is picked.
        case "preview":
          if (previewRef.current.state.entries.length === 0) {
            setFilesOnly(true);
            setOverlay("palette");
          } else {
            const wasOpen = previewRef.current.state.open;
            previewRef.current.toggle();
            if (wasOpen) setTimeout(() => actions.focusTerminal(), 0);
          }
          return true;
        case "openFile":
          setFilesOnly(true);
          setOverlay("palette");
          return true;
        case "capture":
          void runCaptureRef.current();
          return true;
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
          toggleSidebar(v);
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
        // Focus only, never an answer: the lane's target card, or the same card in the drawer when the lane is not on screen (focus
        // mode, the rail). A session that asks in its own terminal is gone to instead.
        case "nextWaiting": {
          const next = nextWaiting(requestsRef.current);
          if (next === null) announce("Nothing needs you");
          else if (next.kind === "terminal") jumpToRequestRef.current(next.request);
          else {
            const inLane = document.querySelector<HTMLElement>('.lane .approve-card[data-phase="pending"]');
            if (inLane) inLane.focus();
            else {
              setDrawerId(next.request.id);
              setTimeout(() => document.querySelector<HTMLElement>('.drawer .approve-card[data-phase="pending"]')?.focus(), 0);
            }
          }
          return true;
        }
        default:
          return false;
      }
    },
    [dispatch, announce, sendNav, toggleSidebar],
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
    if (!os || launcherOpen || overlay !== null || review !== null || closeAsk !== null) return;
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
  }, [os, launcherOpen, overlay, review, closeAsk, actions, jumpPrompt, paneCommand]);

  // A palette row: a session to go to, the profiles page, or a command the window runs (LivePalette lists only those).
  const runFromPalette = (id: string) => {
    setOverlay(id === "profiles" || id === "appearance" || id === "agents" || id === "notifications" || id === "diagnostics" ? id : id === "cheatsheet" ? "shortcuts" : null);
    if (id.startsWith("session:")) return pick(id.slice("session:".length));
    // Palette-only commands. The menu opens after the palette has given the focus back, so it keeps the focus it takes.
    if (id === "pause-notifications") return void togglePause();
    if (id === "unsplit") return dispatch({ type: "remove", index: focusIdxRef.current });
    if (id === "layout") return void setTimeout(() => setLayoutMenu(true), 0);
    if (id === "group-by-project") return dispatch({ type: "toggleGrouped" });
    if (id === "resume") return void (focusedRow && startResume(focusedRow));
    // After the palette has given focus back, so the confirm bar's Cancel keeps the focus it takes.
    if (id === "pause") return void (focusedId && setTimeout(() => pauseSession(focusedId) === "sent" && actions.focusTerminal(), 0));
    if (id === "continue") return void (focusedId && (void actions.cont(focusedId), actions.focusTerminal()));
    // After the palette has given focus back, so the bar's Cancel keeps the focus it takes. A plain shell has no mode: nothing happens.
    const mode = modeOfItem(id);
    if (mode) return void (focusedId && setTimeout(() => askRestart(focusedId, mode), 0));
    if (id === "copy-recording-path") return void (endpoint && focusedId && copyRecordingPath(endpoint, focusedId, announce));
    // After the palette has given focus back, so the sheet takes it from the right place and returns it there.
    if (id === "review-task") return void setTimeout(() => reviewSession(focusedId), 0);
    // A task whose folder is gone has no session: its sheet (only the remove step) opens for no tab.
    if (id === "clean-up-task") {
      const gone = missingTask(stateRef.current.tasks);
      return void (gone && setTimeout(() => void openReview("", gone), 0));
    }
    const shortcut = commandShortcut(id);
    // These move the focus (or open a pane that takes it), so they wait for the palette to hand it back first.
    if (shortcut?.kind === "nonePane" || shortcut?.kind === "shellPanel" || shortcut?.kind === "preview" || shortcut?.kind === "openFile") return void setTimeout(() => paneCommand(shortcut), 0);
    if (paneCommand(shortcut)) return;
    if (shortcut?.kind === "launcher") setLauncherOpen(true);
    else if (shortcut?.kind === "prompt") jumpPrompt(shortcut.by);
    // After the palette has given focus back, so a rename box or the terminal keeps the focus it is given.
    else if (shortcut && shortcut.kind !== "palette") setTimeout(() => actions.handle(shortcut), 0);
  };

  // Pause notifications: the core owns the flag (it outlives a core restart, so a new endpoint reads it again). The tray, the palette and
  // the Notifications dialog all go through this one toggle. The count is never paused.
  const [paused, setPaused] = useState(false);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;
  useEffect(() => {
    if (!endpoint) return;
    let live = true;
    notify.paused(endpoint).then(
      (p) => live && setPaused(p),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [endpoint]);
  const togglePause = useCallback(async () => {
    const ep = endpointRef.current;
    if (!ep) return;
    const next = !pausedRef.current;
    try {
      await notify.setPaused(ep, next);
      setPaused(next);
      announce(next ? "Notifications paused" : "Notifications resumed");
    } catch {
      announce("Couldn't change the pause. Try again.");
    }
  }, [announce]);
  const trayState = useMemo(() => (os ? { os, sessions: traySessions(entries, rows), paused, doctorIssues: null, departures: false } : null), [os, entries, rows, paused]);
  useTray(trayState);
  // A tray menu choice. The shell has already brought the window forward for the ones that need it.
  useTrayActions((action: TrayAction) => {
    if (action.kind === "goto") actions.handle({ kind: "goto", number: action.number });
    else if (action.kind === "nextWaiting") paneCommand({ kind: "nextWaiting" });
    else if (action.kind === "overlay") setOverlay(action.overlay);
    else void togglePause();
  });
  // The close button with no choice saved: the shell keeps the window open and asks for the one-time dialog.
  const closeAskOpen = useRef(false);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    listen("close-choice", () => {
      closeAskOpen.current = true;
      setCloseAsk({ busy: false, error: null });
    })
      .then((s) => (gone ? s() : (stop = s)))
      .catch(() => {}); // Outside Tauri (tests, a plain browser) there are no events.
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
  const cancelCloseAsk = () => {
    closeAskOpen.current = false;
    setCloseAsk(null);
  };
  // Saves the choice, waits for it, and only then closes: the shell reads the saved choice on the close request.
  const chooseClose = async (keepInTray: boolean) => {
    if (!sync) return setCloseAsk({ busy: false, error: "Couldn't save that choice. Try again." });
    setCloseAsk({ busy: true, error: null });
    try {
      await sync.setCloseToTray(keepInTray);
    } catch {
      if (closeAskOpen.current) setCloseAsk({ busy: false, error: "Couldn't save that choice. Try again." });
      return;
    }
    // Escape while it saved: the choice stands, but the person asked not to close now.
    if (!closeAskOpen.current) return;
    closeAskOpen.current = false;
    setCloseAsk(null);
    void closeWindow();
  };

  const restartInfo = restarting === null ? undefined : infos.find((i) => i.tab_id === restarting.id);
  const pauseInfo = pauseBar === null || restartInfo ? undefined : infos.find((i) => i.tab_id === pauseBar.id);
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
      const info = infos.find((i) => i.tab_id === row.id);
      // While Resume is out the core already says Running, but the strip stays and the terminal waits (see `resuming` in sessionActions).
      const bar = heldRow(row) ?? (offersResume(row, info) ? row : undefined);
      const hasTerminal = bar === undefined || bar.phrase !== ENDED_PHRASE;
      const profile = profiles.find((p) => p.id === info?.profile_id);
      const profileGone = info && resumeBlockedReason(info.profile_id, profiles);
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
                paused={info?.paused === true}
                onBlockedInput={() => actions.showPausedBar(row.id)}
                focusToken={focusToken}
                promptJump={promptJump?.tabId === row.id ? promptJump : undefined}
                onPromptJump={(result) => promptJump && onPromptJump(promptJump.by, result)}
                onImagePaste={(image) => void addImage(row.id, image)}
                theme={themeFor(overrideOf(row.id))}
                font={terminalFont}
              />
            </div>
          )}
          {bar && info && (
            <ResumeBar
              row={bar}
              command={sessionResumeCommand(info, profile?.name ?? info.profile_id)}
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
  // The preview pane for the current entry, drawn in the focused session's window. It follows the theme of the terminal beside it.
  const previewPane =
    previewEntry && os ? (
      <Preview
        entry={previewEntry}
        theme={resolveTerminalTheme({ ...themeFor(overrideOf(focusedId ?? "")), app: appScheme() }).theme}
        os={os}
        canBack={preview.state.index > 0}
        canForward={preview.state.index < preview.state.entries.length - 1}
        onBack={preview.back}
        onForward={preview.forward}
        onOpen={preview.open}
        onView={preview.setView}
        onClose={() => {
          preview.close();
          setTimeout(() => actions.focusTerminal(), 0);
        }}
        onEscape={() => actions.focusTerminal()}
        regionTarget={deliveryFor(focusedRow ?? null)}
        onRegion={addRegion}
      />
    ) : undefined;
  // The core decides whether a banner is worth showing from this: whether the window has focus and which sessions are on screen.
  useViewReport(endpoint, previewPane && focusedId ? [focusedId] : visibleIds);
  return (
    <PreviewContext.Provider value={preview.open}>
    <div ref={ref} className="live-app">
      {endpoint && settled && loaded && (
        <MainWindow
          needsYou={needsYou}
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
          visibleIds={previewPane && focusedId ? [focusedId] : visibleIds}
          preview={previewPane}
          os={os}
          windowControls={{ maximized, onMinimize: () => void minimizeWindow(), onToggleMaximize: () => void toggleMaximize(), onClose: () => void closeWindow() }}
          requests={requests}
          renderCard={(r) => liveCard(r)}
          onJumpRequest={jumpToRequest}
          readScreen={endpoint ? (id) => peekScreen(endpoint, id) : undefined}
          onPeekClose={() => actions.focusTerminal()}
          drawer={drawerRequest !== undefined}
          drawerCard={drawerRequest && liveCard(drawerRequest, true)}
          onDrawerClose={() => setDrawerId(undefined)}
          ports={[]}
          footer={footer}
          shell={{ terminal: shellTerminal, cwd: panelTab?.folder, hidden: view.shell_hidden, failed: shellFailed, onRetry: retryShell, openDisabled: !panelTab, forceOpen: shellForced, onToggle: toggleShell, onOpenInPane: () => void openInPane() }}
          composer={false}
          statusBar={false}
          renderTerminal={renderTerminal}
          paneBar={
            restartInfo
              ? { id: restartInfo.tab_id, node: <RestartBar text={restartBarText(restartInfo, restarting!.mode, rows.find((r) => r.id === restartInfo.tab_id)?.status === "working")} onCancel={actions.cancelRestart.bind(actions)} onRestart={() => void actions.confirmRestart()} /> }
              : pauseInfo && pauseBar
                ? {
                    id: pauseInfo.tab_id,
                    node:
                      pauseBar.kind === "confirm" ? (
                        <PauseBar text={pauseBarText(pauseInfo)} action="Pause" onCancel={actions.cancelPause.bind(actions)} onAction={() => void actions.confirmPause()} />
                      ) : (
                        <PauseBar text={PAUSED_TYPING_TEXT} action="Continue" cancel="Dismiss" onCancel={actions.cancelPause.bind(actions)} onAction={() => void actions.continueFromBar()} />
                      ),
                  }
                : undefined
          }
          onModeMenu={(id, anchor) => setModeMenu({ id, anchor, from: "chip" })}
          canChangeMode={(row) => offersModeChange(row, infos.find((i) => i.tab_id === row.id))}
          headerBar={closingInfo && <CloseBar text={closeBarText(closingInfo)} onCancel={actions.cancelClose.bind(actions)} onCloseNow={() => void actions.confirmClose()} />}
          toast={toast && <Toast text={toast.text} actionLabel={toast.reopenId ? "Reopen" : undefined} onAction={() => void actions.reopenFromToast()} />}
          notice={toast ? { id: toast.id, text: toast.text } : notice}
          renamingId={renaming}
          onRename={(id, name) => void actions.commitRename(id, name)}
          onRenameCancel={(opts) => actions.cancelRename(opts)}
          onReorder={(ids) => actions.reorder(ids)}
          grouped={view.grouped}
          onSelect={pick}
          onToggle={togglePane}
          onReview={reviewSession}
          onSetup={() => {
            setAgentsFrom("row");
            setOverlay("agents");
          }}
          menuItems={os ? menuItems : undefined}
          onMenuRun={onMenuRun}
          onPalette={() => setOverlay("palette")}
          onToggleSidebar={() => toggleSidebar(view)}
          noneSelected={nav.noneSelected}
          navCursor={nav.cursor}
        />
      )}
      {modeMenu && (
        <ContextMenu
          label={`Mode for ${infos.find((i) => i.tab_id === modeMenu.id)?.name ?? "the session"}`}
          items={modeMenuItems(infos.find((i) => i.tab_id === modeMenu.id)?.mode ?? undefined)}
          anchor={modeMenu.anchor}
          onRun={(itemId) => {
            setModeMenu(null);
            const mode = modeOfItem(itemId);
            // The current mode, or a session that can't change: nothing opens, so the focus goes back where it was.
            if (!mode || !askRestart(modeMenu.id, mode)) returnFromModeMenu(modeMenu);
          }}
          onClose={({ refocus }) => {
            setModeMenu(null);
            if (refocus) returnFromModeMenu(modeMenu);
          }}
        />
      )}
      {overlay === "palette" && os && <LivePalette sessions={paletteSessions(rows)} os={os} labels={paletteLabels} hidden={hiddenCommands} tab={focusedId ?? null} filesOnly={filesOnly} onOpenFile={(path) => { setOverlay(null); preview.open({ path }); }} onRun={runFromPalette} onClose={() => setOverlay(null)} />}
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
      {overlay === "agents" && endpoint && <AgentsDialog endpoint={endpoint} sync={sync ?? undefined} startWith={agentsFrom === "row" ? "setup" : undefined} onClose={() => { setOverlay(null); setAgentsFrom(null); refreshSetup(); }} onNotice={announce} onChanged={refreshSetup} onNotifications={() => { setAgentsFrom(null); setOverlay("notifications"); }} />}
      {overlay === "notifications" && <NotificationsDialog sync={sync ?? undefined} onClose={() => setOverlay(null)} />}
      {overlay === "diagnostics" && endpoint && <DiagnosticsDialog endpoint={endpoint} onClose={() => setOverlay(null)} onNotice={announce} />}
      {onboarding && endpoint && (
        <div className="live-app__onboarding">
          <LiveOnboarding
            endpoint={endpoint}
            onDone={() => {
              setOnboardingShown("done");
              refreshSetup();
              void sync?.markOnboarded().catch(() => {});
            }}
          />
        </div>
      )}
      {overlay === "appearance" && <AppearanceDialog onClose={() => setOverlay(null)} onNotice={announce} />}
      {overlay === "shortcuts" && os && <LiveShortcuts os={os} onClose={() => setOverlay(null)} />}
      {closeAsk && <CloseDialog busy={closeAsk.busy} error={closeAsk.error} onChoose={(keep) => void chooseClose(keep)} onCancel={cancelCloseAsk} />}
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
    </PreviewContext.Provider>
  );
}
