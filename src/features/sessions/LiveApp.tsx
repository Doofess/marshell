import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import { sessions } from "../../lib/api/sessions";
import { cancelsCloseBar, commandShortcut, sessionShortcut, type SessionShortcut } from "../../lib/appShortcuts";
import { osOfPlatform } from "../../lib/keymap";
import { paletteSessions } from "../palette/sessions";
import { ProfilesDialog } from "../settings/ProfilesPage";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import type { RowModel } from "../sidebar/types";
import { AppearanceDialog } from "../appearance/AppearanceDialog";
import { useAppearance } from "../appearance/AppearanceProvider";
import { terminalThemeOf, wtFont } from "../appearance/terminalSettings";
import { jumpWords } from "../terminal/promptMarks";
import { resolveTerminalFont } from "../terminal/terminalFont";
import { TerminalView, type TerminalThemeProp } from "../terminal/TerminalView";
import { sidebarMode } from "../window/layout";
import { MainWindow } from "../window/MainWindow";
import type { Arrangement } from "../window/splitLayout";
import { useViewState } from "../window/useViewState";
import { fillsEmptyPane, focusedIndex, railToggle, ratiosOf, sidebarPrefOf } from "../window/viewState";
import { CloseBar } from "./CloseBar";
import { LiveLauncher } from "./LiveLauncher";
import { LivePalette } from "./LivePalette";
import { ResumeBar } from "./ResumeBar";
import { resumeCommand } from "./resumeCommand";
import { SessionActions, closeBarText } from "./sessionActions";
import { Toast } from "./Toast";
import { pruneClosed, without, type StartingTexts } from "./startingTexts";
import { ENDED_PHRASE } from "./toRow";
import { useSessions } from "./useSessions";

/** The size a new session starts at until a terminal has been measured. */
const DEFAULT_SIZE = { cols: 120, rows: 30 };
/** The palette leaves Resume out unless the focused session is Ended or Exited. A constant, so the palette's command list is not rebuilt for it. */
const HIDE_RESUME = ["resume"];
/** The core's id for a session started outside any saved shell profile; it never has a profile to lose. */
const CUSTOM_PROFILE = "custom";

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
  const { endpoint, profiles, rows, infos, selectedId, loaded, restarts, select, reloadProfiles, error } = useSessions();
  // The window's saved layout: which sessions are in which pane. `undefined` until the list has answered, so an empty first list is never "everything closed".
  const [view, dispatch, , settled] = useViewState(endpoint, loaded ? infos.map((i) => i.tab_id) : undefined);
  const viewRef = useRef(view);
  viewRef.current = view;
  // The session the window is about is the one in the focused pane; the store's choice only stands in for an empty pane.
  // A small window shows fewer panes than the layout holds; the focus is the pane it shows, never one out of sight (MainWindow reports how many).
  const [shownCount, setShownCount] = useState<number | undefined>(undefined);
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
      dispatch({ type: "select", tabId: id });
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
  const [overlay, setOverlay] = useState<"palette" | "profiles" | "appearance" | null>(null);
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

  // No sessions (first run, everything closed): the launcher opens. So does a core restart, even before its first list arrives.
  useEffect(() => {
    if (loaded && rows.length === 0) setLauncherOpen(true);
  }, [loaded, rows.length]);
  useEffect(() => {
    if (restarts > 0) setLauncherOpen(true);
  }, [restarts]);

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
  const paletteLabels = useMemo(() => ({ sidebar: sidebarPref === "expanded" ? "Collapse sidebar to the rail" : "Expand the sidebar" }), [sidebarPref]);
  const context = useRef({ endpoint, infos, selectedId: focusedId, sidebarExpanded });
  context.current = { endpoint, infos, selectedId: focusedId, sidebarExpanded };
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
  // The Ended or Exited row each tab had when Resume was pressed. While the request is out the core already says Running, but the strip
  // (and the button that has the focus) must stay put until the answer comes, so it is drawn from this.
  const resumeRows = useRef(new Map<string, RowModel>());
  const startResume = useCallback(
    (row: RowModel) => {
      if (row.status === "ended") resumeRows.current.set(row.id, row);
      void actions.resume(row.id);
    },
    [actions],
  );
  const focusedRow = rows.find((r) => r.id === focusedId);
  const resumable = focusedRow?.status === "ended";
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

  // The split view's shortcuts and commands. True when it was one of them.
  const paneCommand = useCallback(
    (shortcut: SessionShortcut | null): boolean => {
      const v = viewRef.current;
      switch (shortcut?.kind) {
        case "split":
          // A zoomed pane would hide the one being added.
          if (v.zoomed !== null) dispatch({ type: "zoom" });
          dispatch({ type: "split" });
          return true;
        case "zoom":
          if (v.zoomed !== null) dispatch({ type: "zoom" });
          else if (v.panes.length > 1 && v.panes[focusIdxRef.current] !== null) {
            // The zoom applies to the active pane, so the one in view is made active first.
            dispatch({ type: "focusPane", index: focusIdxRef.current });
            dispatch({ type: "zoom" });
          }
          return true;
        case "nextPane":
          dispatch({ type: "nextPane" });
          return true;
        case "focusMode":
          dispatch({ type: "toggleFocus" });
          return true;
        case "sidebar":
          dispatch({ type: "sidebar", pref: railToggle(sidebarPrefOf(v)) });
          return true;
        case "nonePane":
          // No terminal selected: the keys stop going to a program until a pane is chosen again.
          if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
          document.querySelector<HTMLElement>(".window__main")?.focus();
          return true;
        default:
          return false;
      }
    },
    [dispatch],
  );

  // The app's shortcuts work from anywhere, the terminal included; the terminal lets these keys through (TerminalView).
  const os = endpoint ? osOfPlatform(endpoint.platform) : undefined;
  useEffect(() => {
    if (!os || launcherOpen || overlay !== null) return;
    const onKey = (e: KeyboardEvent) => {
      const shortcut = sessionShortcut(e, os);
      if (!shortcut) return;
      // A text box (the rename box, the launcher's fields) keeps its own keys; xterm's own hidden textarea is the terminal, not a box.
      const t = e.target;
      if ((t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement) && !t.classList.contains("xterm-helper-textarea")) return;
      e.preventDefault();
      // Holding the key repeats it; only moving a session along the list, or from prompt to prompt, should.
      if (e.repeat && shortcut.kind !== "move" && shortcut.kind !== "prompt") return;
      if (shortcut.kind === "launcher") setLauncherOpen(true);
      else if (shortcut.kind === "palette") setOverlay("palette");
      else if (shortcut.kind === "prompt") jumpPrompt(shortcut.by);
      else if (!paneCommand(shortcut)) actions.handle(shortcut);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [os, launcherOpen, overlay, actions, jumpPrompt, paneCommand]);

  // A palette row: a session to go to, the profiles page, or a command the window runs (LivePalette lists only those).
  const runFromPalette = (id: string) => {
    setOverlay(id === "profiles" || id === "appearance" ? id : null);
    if (id.startsWith("session:")) return pick(id.slice("session:".length));
    // Palette-only commands. The menu opens after the palette has given the focus back, so it keeps the focus it takes.
    if (id === "unsplit") return dispatch({ type: "remove", index: focusIdxRef.current });
    if (id === "layout") return void setTimeout(() => setLayoutMenu(true), 0);
    if (id === "resume") return void (focusedRow && startResume(focusedRow));
    const shortcut = commandShortcut(id);
    if (shortcut?.kind === "nonePane") return void setTimeout(() => paneCommand(shortcut), 0);
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
  };

  // One tab's content, in the single window and in every split pane: its terminal, and under it the Resume strip while it is Ended or
  // Exited. An Ended tab has no terminal at all. The wrapper is the same for every state, so a session that exits keeps its terminal
  // (and its scrollback) mounted when the strip appears.
  const renderTerminal = useCallback(
    (row: RowModel, shown = true) => {
      if (!endpoint) return null;
      // While Resume is out the core already says Running, but the strip stays and the terminal waits (see `resuming` in sessionActions).
      const held = resuming.includes(row.id) ? resumeRows.current.get(row.id) : undefined;
      const bar = held ?? (row.status === "ended" ? row : undefined);
      const hasTerminal = bar === undefined || bar.phrase !== ENDED_PHRASE;
      const info = infos.find((i) => i.tab_id === row.id);
      const profile = profiles.find((p) => p.id === info?.profile_id);
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
                autoFocus={!launcherOpen && overlay === null && row.id === focusedId}
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
              disabledReason={profile || info.profile_id === CUSTOM_PROFILE ? undefined : "Its shell profile was removed."}
              onResume={() => startResume(bar)}
            />
          )}
        </div>
      );
    },
    [endpoint, resuming, resumed, infos, profiles, startResume, startingTexts, onSize, launcherOpen, overlay, focusedId, focusToken, promptJump, onPromptJump, themeFor, overrideOf, terminalFont],
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
          onLayout={(count, arrangement) => {
            if (count !== viewRef.current.panes.length) dispatch({ type: "count", count });
            dispatch({ type: "arrange", arrangement });
          }}
          onPane={dispatch}
          rows={rows}
          selectedId={focusedId}
          requests={[]}
          ports={[]}
          footer={null}
          shell={false}
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
          onSelect={pick}
          onPalette={() => setOverlay("palette")}
        />
      )}
      {overlay === "palette" && os && <LivePalette sessions={paletteSessions(rows)} os={os} labels={paletteLabels} hidden={resumable ? undefined : HIDE_RESUME} onRun={runFromPalette} onClose={() => setOverlay(null)} />}
      {overlay === "profiles" && endpoint && <ProfilesDialog endpoint={endpoint} onClose={() => setOverlay(null)} onNotice={announce} onSaved={reloadProfiles} />}
      {overlay === "appearance" && <AppearanceDialog onClose={() => setOverlay(null)} onNotice={announce} />}
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
