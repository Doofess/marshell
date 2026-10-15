import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import { findCommand } from "../../lib/bindings";
import { osOfPlatform } from "../../lib/keymap";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import type { RowModel } from "../sidebar/types";
import { TerminalView } from "../terminal/TerminalView";
import { MainWindow } from "../window/MainWindow";
import { LiveLauncher } from "./LiveLauncher";
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

  // Ctrl+Shift+T opens it from anywhere; the terminal lets that key through (TerminalView).
  const os = endpoint ? osOfPlatform(endpoint.platform) : undefined;
  useEffect(() => {
    if (!os) return;
    const onKey = (e: KeyboardEvent) => {
      if (findCommand(e, os)?.id !== "launcher") return;
      e.preventDefault();
      setLauncherOpen(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [os]);

  const started = (created: CreateSessionResponse, text: string) => {
    setStartingTexts((t) => ({ ...t, [created.tab_id]: text }));
    select(created.tab_id);
    setLauncherOpen(false);
  };

  const renderTerminal = useCallback(
    (row: RowModel) =>
      endpoint ? (
        <TerminalView
          key={row.id}
          endpoint={endpoint}
          tabId={row.id}
          placeholder={startingTexts[row.id]}
          onOutput={() => setStartingTexts((t) => without(t, row.id))}
          onSize={onSize}
          autoFocus={!launcherOpen}
        />
      ) : null,
    [endpoint, startingTexts, onSize, launcherOpen],
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
        />
      )}
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
