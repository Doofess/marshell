import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import { sessions } from "../../lib/api/sessions";
import { cancelsCloseBar, commandShortcut, sessionShortcut } from "../../lib/appShortcuts";
import { osOfPlatform } from "../../lib/keymap";
import { paletteSessions } from "../palette/sessions";
import { ProfilesDialog } from "../settings/ProfilesPage";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import type { RowModel } from "../sidebar/types";
import { useAppearance } from "../appearance/AppearanceProvider";
import { terminalThemeOf, wtFont } from "../appearance/terminalSettings";
import { jumpWords } from "../terminal/promptMarks";
import { resolveTerminalFont } from "../terminal/terminalFont";
import { TerminalView, type TerminalThemeProp } from "../terminal/TerminalView";
import { sidebarMode } from "../window/layout";
import { MainWindow } from "../window/MainWindow";
import { CloseBar } from "./CloseBar";
import { LiveLauncher } from "./LiveLauncher";
import { LivePalette } from "./LivePalette";
import { SessionActions, closeBarText } from "./sessionActions";
import { Toast } from "./Toast";
import { pruneClosed, without, type StartingTexts } from "./startingTexts";
import { useSessions } from "./useSessions";

/** The size a new session starts at until a terminal has been measured. */
const DEFAULT_SIZE = { cols: 120, rows: 30 };

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
  const { endpoint, profiles, rows, infos, selectedId, loaded, restarts, select, error } = useSessions();
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
  const [overlay, setOverlay] = useState<"palette" | "profiles" | null>(null);
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
  // Only the expanded sidebar has rows to rename (MainWindow shows the rail or nothing below that).
  const sidebarExpanded = sidebarMode(width, "expanded") === "expanded";
  const context = useRef({ endpoint, infos, selectedId, sidebarExpanded });
  context.current = { endpoint, infos, selectedId, sidebarExpanded };
  const selectRef = useRef(select);
  selectRef.current = select;
  const actions = useMemo(() => new SessionActions({ api: sessions, context: () => context.current, select: (id) => selectRef.current(id) }), []);
  useEffect(() => () => actions.dispose(), [actions]);
  const { toast, closing, renaming, focusToken } = useSyncExternalStore(actions.subscribe, actions.getState);
  // A question about a session that is no longer the selected one, or is gone, has nothing left to ask.
  useEffect(() => {
    if (closing !== null && (closing !== selectedId || !infos.some((i) => i.tab_id === closing))) actions.cancelClose();
  }, [closing, selectedId, infos, actions]);
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

  const selectedRef = useRef(selectedId);
  selectedRef.current = selectedId;
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
      else actions.handle(shortcut);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [os, launcherOpen, overlay, actions, jumpPrompt]);

  // A palette row: a session to go to, the profiles page, or a command the window runs (LivePalette lists only those).
  const runFromPalette = (id: string) => {
    setOverlay(id === "profiles" ? "profiles" : null);
    if (id.startsWith("session:")) return select(id.slice("session:".length));
    const shortcut = commandShortcut(id);
    if (shortcut?.kind === "launcher") setLauncherOpen(true);
    else if (shortcut?.kind === "prompt") jumpPrompt(shortcut.by);
    // After the palette has given focus back, so a rename box or the terminal keeps the focus it is given.
    else if (shortcut && shortcut.kind !== "palette") setTimeout(() => actions.handle(shortcut), 0);
  };

  const closingInfo = closing === null ? undefined : infos.find((i) => i.tab_id === closing);

  const started = (created: CreateSessionResponse, text: string) => {
    setStartingTexts((t) => ({ ...t, [created.tab_id]: text }));
    select(created.tab_id);
    setLauncherOpen(false);
  };

  const renderTerminal = useCallback(
    (row: RowModel, shown = true) =>
      endpoint ? (
        <TerminalView
          key={row.id}
          endpoint={endpoint}
          tabId={row.id}
          placeholder={startingTexts[row.id]}
          onOutput={() => setStartingTexts((t) => without(t, row.id))}
          onSize={onSize}
          autoFocus={!launcherOpen && overlay === null}
          visible={shown}
          focusToken={focusToken}
          promptJump={promptJump?.tabId === row.id ? promptJump : undefined}
          onPromptJump={(result) => promptJump && onPromptJump(promptJump.by, result)}
          theme={themeFor(overrideOf(row.id))}
          font={terminalFont}
        />
      ) : null,
    [endpoint, startingTexts, onSize, launcherOpen, overlay, focusToken, promptJump, onPromptJump, themeFor, overrideOf, terminalFont],
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
      {endpoint && (
        <MainWindow
          width={width}
          height={height}
          layout="default"
          rows={rows}
          selectedId={selectedId}
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
          onSelect={select}
          onPalette={() => setOverlay("palette")}
        />
      )}
      {overlay === "palette" && os && <LivePalette sessions={paletteSessions(rows)} os={os} onRun={runFromPalette} onClose={() => setOverlay(null)} />}
      {overlay === "profiles" && endpoint && <ProfilesDialog endpoint={endpoint} onClose={() => setOverlay(null)} onNotice={announce} />}
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
