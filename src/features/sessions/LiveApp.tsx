import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import { sessions } from "../../lib/api/sessions";
import { cancelsCloseBar, commandShortcut, sessionShortcut } from "../../lib/appShortcuts";
import { osOfPlatform } from "../../lib/keymap";
import { paletteSessions } from "../palette/sessions";
import { ProfilesDialog } from "../settings/ProfilesPage";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import type { RowModel } from "../sidebar/types";
import { TerminalView } from "../terminal/TerminalView";
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
  const { endpoint, rows, infos, selectedId, loaded, restarts, select, error } = useSessions();
  const { ref, width, height } = useElementSize();
  const [launcherOpen, setLauncherOpen] = useState(false);
  // The command palette, or the shell profiles page opened from it, over the window.
  const [overlay, setOverlay] = useState<"palette" | "profiles" | null>(null);
  // What the profiles page says aloud through the window's announcer. Negative ids never meet a toast's.
  const [profilesNotice, setProfilesNotice] = useState<{ id: number; text: string } | null>(null);
  const noticeCount = useRef(0);
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
      // Holding the key repeats it; only moving a session along the list should.
      if (e.repeat && shortcut.kind !== "move") return;
      if (shortcut.kind === "launcher") setLauncherOpen(true);
      else if (shortcut.kind === "palette") setOverlay("palette");
      else actions.handle(shortcut);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [os, launcherOpen, overlay, actions]);

  // A palette row: a session to go to, the profiles page, or a command the window runs (LivePalette lists only those).
  const runFromPalette = (id: string) => {
    setOverlay(id === "profiles" ? "profiles" : null);
    if (id.startsWith("session:")) return select(id.slice("session:".length));
    const shortcut = commandShortcut(id);
    if (shortcut?.kind === "launcher") setLauncherOpen(true);
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
        />
      ) : null,
    [endpoint, startingTexts, onSize, launcherOpen, overlay, focusToken],
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
          notice={toast ? { id: toast.id, text: toast.text } : profilesNotice}
          renamingId={renaming}
          onRename={(id, name) => void actions.commitRename(id, name)}
          onRenameCancel={(opts) => actions.cancelRename(opts)}
          onReorder={(ids) => actions.reorder(ids)}
          onSelect={select}
          onPalette={() => setOverlay("palette")}
        />
      )}
      {overlay === "palette" && os && <LivePalette sessions={paletteSessions(rows)} os={os} onRun={runFromPalette} onClose={() => setOverlay(null)} />}
      {overlay === "profiles" && endpoint && <ProfilesDialog endpoint={endpoint} onClose={() => setOverlay(null)} onNotice={(text) => setProfilesNotice({ id: -++noticeCount.current, text })} />}
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
