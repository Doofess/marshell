import { useEffect, useState } from "react";
import { type StartResult, startSession } from "./lib/api/session";
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

  // Startup is well under 150 ms, so no spinner (UX gate: loading states only past 150 ms).
  if (state === null) return null;
  if (state.kind === "error") {
    return (
      <p className="startup-error" role="alert">
        {state.message}
      </p>
    );
  }
  return <TerminalView endpoint={state.endpoint} tabId={state.tabId} />;
}
