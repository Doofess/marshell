import { useEffect, useRef, useState } from "react";
import type { DoctorSummary } from "../../generated/DoctorSummary";
import type { Endpoint } from "../../generated/Endpoint";
import { cliFinishedSince } from "./cliDoctors";
import type { DoctorReport } from "./types";

export type ClickRunOptions = {
  endpoint: Endpoint | null;
  report: DoctorReport | null;
  /** The core says the run is going (the summary's flag). */
  running: boolean;
  /** When the run last finished, as the report carries it. */
  at: (r: DoctorReport) => number | undefined;
  start: (ep: Endpoint) => Promise<DoctorSummary>;
  isRunning: (s: DoctorSummary) => boolean;
  load: () => Promise<void>;
  announce: (text: string) => void;
  done: (r: DoctorReport) => string;
  failed: (message: string) => string;
};

const messageOf = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * A button that starts a run and stays busy from the click until the report carries a newer run. The summary
 * socket can skip a quick run's frames, so the report decides, not a seen `running` change.
 */
export function useClickRun({ endpoint, report, running, at, start, isRunning, load, announce, done, failed }: ClickRunOptions): { pending: boolean; run: () => Promise<void> } {
  const [pending, setPending] = useState(false);
  const reportRef = useRef(report);
  reportRef.current = report;
  const base = useRef<number | undefined>(undefined);
  const awaiting = useRef(false);
  // A run started elsewhere while the page shows a report is announced too. One already going when the page opened
  // isn't: the report it would be compared with isn't in yet.
  // A seen end clears the button too, for a run that failed and left the report's time as it was.
  const wasRunning = useRef(running);
  useEffect(() => {
    if (!running && wasRunning.current) setPending(false);
    wasRunning.current = running;
    if (!running || awaiting.current || !reportRef.current) return;
    base.current = at(reportRef.current);
    awaiting.current = true;
  }, [running]);
  useEffect(() => {
    if (!awaiting.current || !report || !cliFinishedSince(base.current, at(report))) return;
    awaiting.current = false;
    setPending(false);
    announce(done(report));
  }, [report, announce]);

  const run = async () => {
    if (!endpoint) return;
    base.current = report ? at(report) : undefined;
    awaiting.current = true;
    setPending(true);
    try {
      const s = await start(endpoint);
      // Already over by the time the answer came: load the report that has the new items.
      if (!isRunning(s)) await load();
    } catch (e) {
      awaiting.current = false;
      setPending(false);
      announce(failed(messageOf(e)));
    }
  };

  return { pending, run };
}
