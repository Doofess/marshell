import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { Launcher } from "../launcher/Launcher";
import type { LauncherChoice } from "../launcher/launchChoice";
import type { RowModel } from "../sidebar/types";
import { TerminalView } from "../terminal/TerminalView";
import { MainWindow } from "../window/MainWindow";
import { useSessions } from "./useSessions";

const LAUNCHER_CHOICE: LauncherChoice = { program: "claude", projectId: "", mode: "Manual", pace: "Balanced", worktree: false };

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

/** The real app: the main window on the core's live session list. */
export function LiveApp() {
  const { endpoint, rows, selectedId, loaded, error } = useSessions();
  const { ref, width, height } = useElementSize();
  const renderTerminal = useCallback(
    (row: RowModel) => (endpoint ? <TerminalView key={row.id} endpoint={endpoint} tabId={row.id} /> : null),
    [endpoint],
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
          shell={false}
          composer={false}
          statusBar={false}
          renderTerminal={renderTerminal}
        />
      )}
      {/* No sessions (first run, everything closed, or a core restart): the launcher is open. */}
      {loaded && rows.length === 0 && (
        <div className="live-app__launcher">
          <Launcher view={{ choice: LAUNCHER_CHOICE, projects: [], query: null, activeIndex: 0, nextNumber: 1 }} />
        </div>
      )}
    </div>
  );
}
