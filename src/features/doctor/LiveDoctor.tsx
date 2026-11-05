import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { DoctorSummary } from "../../generated/DoctorSummary";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import { applyFix, doctorReport, lastFix, planFix, runCliDoctors, runDoctor, undoDifferences, undoFix } from "../../lib/api/doctor";
import { ApiError } from "../../lib/api/client";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { needsBypassAck, type SessionActions } from "../sessions/sessionActions";
import { DoctorPage } from "./DoctorPage";
import { doctorReducer, INITIAL_DOCTOR } from "./doctorModel";
import { cliDoneAnnouncement, cliFinishedSince } from "./cliDoctors";
import { planRestarts, skippedReceipt } from "./doctorRestart";
import { fixAnnouncement, undoRefusedAnnouncement } from "./doctorUi";
import { selectedFixable } from "./report";
import { DifferencesSheet, FixSheet } from "./FixSheet";
import "./LiveDoctor.css";

const message = (e: unknown) => (e instanceof ApiError ? e.message : e instanceof Error ? e.message : String(e));
const plural = (n: number) => (n === 1 ? "session" : "sessions");

export type LiveDoctorProps = {
  endpoint: Endpoint | null;
  /** The latest `doctor` frame; a finished one (a newer `checked_at_ms`) reloads the report while the page is open. */
  summary: DoctorSummary | null;
  infos: readonly SessionInfo[];
  /** Whether the session is working right now; "Restart them" leaves such a session alone. */
  isWorking: (tabId: string) => boolean;
  actions: SessionActions;
  /** The window's announcer. */
  announce: (text: string) => void;
  onOpenAgents: () => void;
  onClose: () => void;
};

/**
 * The Doctor over the live window: opening it runs a check, and everything it changes goes through the core's preview and apply.
 * The page, the fix sheet and the toast are Task 7's components; this keeps their state (`doctorReducer`) and makes the calls.
 */
export function LiveDoctor({ endpoint, summary, infos, isWorking, actions, announce, onOpenAgents, onClose }: LiveDoctorProps) {
  const [m, dispatch] = useReducer(doctorReducer, INITIAL_DOCTOR);
  const [diff, setDiff] = useState<ConfigPreview | null>(null);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const startedAt = useRef(Date.now());
  const root = useRef<HTMLDivElement>(null);
  const sheetOpen = useRef(false);
  sheetOpen.current = m.plan !== null || diff !== null;
  useDialogFocus(root, root, () => {
    if (!sheetOpen.current) onClose();
  });

  useEffect(() => {
    const id = setInterval(() => setNowMs(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const load = useCallback(async () => {
    if (!endpoint) return;
    try {
      const report = await doctorReport(endpoint);
      if (report) dispatch({ type: "reportLoaded", report });
    } catch (e) {
      dispatch({ type: "runFailed", message: message(e) });
    }
  }, [endpoint]);

  const run = useCallback(async () => {
    if (!endpoint) return;
    startedAt.current = Date.now();
    setNowMs(Date.now());
    dispatch({ type: "runStarted" });
    try {
      const s = await runDoctor(endpoint);
      if (!s.running) await load();
    } catch (e) {
      dispatch({ type: "runFailed", message: message(e) });
    }
  }, [endpoint, load]);

  // Opening the page runs the check and reads the last fix, so Undo last fix is in the header after a restart.
  const seen = useRef(summary?.checked_at_ms);
  useEffect(() => {
    void run();
    if (endpoint) lastFix(endpoint).then((fix) => dispatch({ type: "undoLoaded", fix }), () => {});
  }, [endpoint, run]);

  // A run that finishes while the page is open (a newer `checked_at_ms`) loads its report.
  useEffect(() => {
    const at = summary?.checked_at_ms;
    if (summary === null || summary.running || at === undefined || (seen.current !== undefined && at <= seen.current)) return;
    seen.current = at;
    void load();
  }, [summary, load]);

  // The CLI doctors: the button stays busy from the click until the report carries a newer CLI run. The summary
  // socket can skip a quick run's frames, so the report decides, not a seen `cli_running` change.
  const [cliPending, setCliPending] = useState(false);
  const reportRef = useRef(m.report);
  reportRef.current = m.report;
  const cliBase = useRef<number | undefined>(undefined);
  const cliAwaiting = useRef(false);
  const cliNow = summary?.cli_running ?? false;
  // A run started elsewhere while the page shows a report is announced too. One already going when the page opened
  // isn't: the report it would be compared with isn't in yet.
  // A seen end clears the button too, for a run that failed and left the report's time as it was.
  const wasCliRunning = useRef(cliNow);
  useEffect(() => {
    if (!cliNow && wasCliRunning.current) setCliPending(false);
    wasCliRunning.current = cliNow;
    if (!cliNow || cliAwaiting.current || !reportRef.current) return;
    cliBase.current = reportRef.current.cli_checked_at_ms;
    cliAwaiting.current = true;
  }, [cliNow]);
  useEffect(() => {
    if (!cliAwaiting.current || !m.report || !cliFinishedSince(cliBase.current, m.report.cli_checked_at_ms)) return;
    cliAwaiting.current = false;
    setCliPending(false);
    announce(cliDoneAnnouncement(m.report));
  }, [m.report, announce]);

  const runCli = async () => {
    if (!endpoint) return;
    cliBase.current = m.report?.cli_checked_at_ms;
    cliAwaiting.current = true;
    setCliPending(true);
    try {
      const s = await runCliDoctors(endpoint);
      // Already over by the time the answer came: load the report that has the new items.
      if (!s.cli_running) await load();
    } catch (e) {
      cliAwaiting.current = false;
      setCliPending(false);
      announce(`Couldn't run the CLI doctors: ${message(e)}`);
    }
  };

  const fix = async () => {
    if (!endpoint || !m.report) return;
    const ids = selectedFixable(m.report, m.selected).map((i) => i.id);
    try {
      dispatch({ type: "planned", plan: await planFix(endpoint, ids) });
    } catch (e) {
      announce(`Couldn't prepare the fix: ${message(e)}`);
      void run();
    }
  };

  const confirm = async () => {
    if (!endpoint || !m.plan) return;
    dispatch({ type: "fixStarted" });
    try {
      const r = await applyFix(endpoint, m.plan.preview);
      if (r.ok) {
        dispatch({ type: "fixApplied", fix: r.fix });
        announce(fixAnnouncement(r.fix).text);
      } else if (r.conflict) dispatch({ type: "fixConflict" });
      else {
        dispatch({ type: "fixFailed" });
        announce(r.message);
      }
    } catch (e) {
      dispatch({ type: "fixFailed" });
      announce(`Couldn't apply the fix: ${message(e)}`);
    }
  };

  const reload = async () => {
    if (!endpoint || !m.plan) return;
    try {
      dispatch({ type: "planned", plan: await planFix(endpoint, m.plan.item_ids) });
    } catch (e) {
      dispatch({ type: "cancelPlan" });
      announce(`Couldn't prepare the fix: ${message(e)}`);
      void run();
    }
  };

  const undo = async () => {
    if (!endpoint) return;
    try {
      const r = await undoFix(endpoint);
      if (r.ok) {
        dispatch({ type: "undone" });
        announce("The last fix is undone.");
        void run();
      } else if ("changed" in r) {
        dispatch({ type: "undoStale" });
        announce(undoRefusedAnnouncement(r.changed).text);
      } else announce(r.message);
    } catch (e) {
      announce(`Couldn't undo the last fix: ${message(e)}`);
    }
  };

  const showDifferences = async () => {
    if (!endpoint || !m.undo) return;
    try {
      setDiff(await undoDifferences(endpoint, m.undo.snapshot_id));
    } catch (e) {
      announce(`Couldn't show the differences: ${message(e)}`);
    }
  };

  const restart = async () => {
    if (!m.undo) return;
    const { go, skipped } = planRestarts(m.undo.restart, infos, isWorking, (i) => i.mode !== null && needsBypassAck(i, i.mode, actions.sessionSkipsAll(i)));
    dispatch({ type: "undoLoaded", fix: { ...m.undo, restart: [] } });
    const done = await Promise.all(go.map((g) => actions.restartNow(g.tabId, g.mode, g.elevated)));
    const failed = go.filter((_, i) => !done[i]).map((g) => g.number);
    const restarted = done.filter(Boolean).length;
    const parts = [restarted > 0 ? `Restarted ${restarted} ${plural(restarted)}.` : null, skippedReceipt([...skipped, ...failed])].filter((p) => p !== null);
    if (parts.length > 0) announce(parts.join(" "));
  };

  return (
    <div ref={root} className="live-doctor" role="dialog" aria-modal="true" aria-label="Doctor" tabIndex={-1}>
      <div className="live-doctor__bar">
        <button type="button" className="live-doctor__close" onClick={onClose}>
          Close
        </button>
      </div>
      <DoctorPage
        report={m.report}
        phase={m.phase}
        selected={m.selected}
        undo={m.undo ?? undefined}
        nowMs={nowMs}
        elapsedMs={nowMs - startedAt.current}
        failure={m.failure}
        onToggle={(id) => dispatch({ type: "toggle", id })}
        onFix={() => void fix()}
        onRunAgain={() => void run()}
        cliRunning={cliPending || !!summary?.cli_running}
        onRunCliDoctors={() => void runCli()}
        onUndo={() => void undo()}
        onShowDifferences={() => void showDifferences()}
        onOpenAgents={onOpenAgents}
        onRestart={() => void restart()}
      />
      {m.plan && (
        <div className="live-doctor__sheet">
          <FixSheet plan={m.plan} busy={m.phase === "fixing"} conflict={m.conflict} onConfirm={() => void confirm()} onCancel={() => dispatch({ type: "cancelPlan" })} onReload={() => void reload()} />
        </div>
      )}
      {diff && (
        <div className="live-doctor__sheet">
          <DifferencesSheet preview={diff} onClose={() => setDiff(null)} />
        </div>
      )}
    </div>
  );
}
