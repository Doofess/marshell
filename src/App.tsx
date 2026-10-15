import { useCallback, useEffect, useState } from "react";
import { forgetSession, type StartResult, startSession } from "./lib/api/session";
import { TerminalView } from "./features/terminal/TerminalView";

export function App() {
  const [state, setState] = useState<StartResult | null>(null);
  useEffect(() => {
    let live = true;
    void startSession().then((s) => live && setState(s));
    return () => {
      live = false;
    };
  }, []);
  // After a core restart: a new tab on the new core, in a fresh TerminalView (new key).
  const newShell = useCallback(() => {
    forgetSession();
    void startSession().then(setState);
  }, []);

  // Startup is well under 150 ms, so no spinner (UX gate: loading states only past 150 ms).
  if (state === null) return null;
  if (state.kind === "error") {
    return (
      <p className="startup-error" role="alert">
        {state.message}
      </p>
    );
  }
  return <TerminalView key={state.tabId} endpoint={state.endpoint} tabId={state.tabId} onNewShell={newShell} />;
}
