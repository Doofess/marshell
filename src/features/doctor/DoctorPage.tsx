import { useId } from "react";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import { EMPTY_STATES } from "../onboarding/emptyStates";
import { DoctorRow } from "./DoctorRow";
import { UndoToast } from "./UndoToast";
import { showProgress } from "./doctorUi";
import { counts, fixHeadline, groups, selectedFixable } from "./report";
import type { DoctorLastFix, DoctorReport } from "./types";
import { undoOffer, undoState } from "./undo";
import "./Doctor.css";

export type DoctorPhase = "running" | "ready" | "fixing" | "fixed" | "failed";
const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

export type DoctorPageProps = {
  report: DoctorReport | null;
  phase: DoctorPhase;
  selected: ReadonlySet<string>;
  undo?: DoctorLastFix;
  nowMs: number;
  elapsedMs?: number;
  failure?: string;
  onToggle: (id: string) => void;
  onFix: () => void;
  onRunAgain: () => void;
  onUndo: () => void;
  onShowDifferences: () => void;
  onOpenAgents: () => void;
  /** The toast's "Restart them"; the brief's prop list leaves it out, but the toast needs it. */
  onRestart?: () => void;
};

/**
 * The Doctor page: one report in three groups, one combined fix for the rows that are checked. Nothing is drawn for the first
 * 150 ms of a check; a failure says why and offers Run again; after a fix each fixed row says Fixed in words, the receipt toast
 * floats for 10 seconds and then Undo last fix stays in the header. Info (managed settings) is listed and never has a control.
 */
export function DoctorPage({ report, phase, selected, undo, nowMs, elapsedMs = 0, failure, onToggle, onFix, onRunAgain, onUndo, onShowDifferences, onOpenAgents, onRestart }: DoctorPageProps) {
  const uid = useId();
  const c = report ? counts(report) : { issues: 0, fixable: 0, unfixable: 0 };
  const chosen = report ? selectedFixable(report, selected).length : 0;
  const fixedIds = new Set(phase === "fixed" ? undo?.item_ids : []);
  const fixedCount = report ? report.items.filter((i) => i.group !== "info" && fixedIds.has(i.id)).length : 0;
  const offer = phase === "fixed" && undo ? undoOffer(undoState(undo), nowMs) : undefined;
  const clear = !!report && report.items.length === 0 && phase === "ready";
  const clearCopy = EMPTY_STATES.find((e) => e.id === "doctor-clear")!;
  const checked = report ? `${Math.max(0, Math.round((nowMs - report.checked_at_ms) / 1000))} s ago` : "–";
  const listed = !!report && phase !== "running" && phase !== "failed";

  const actions =
    phase === "running" ? null : (
      <>
        {phase === "ready" && c.fixable > 0 && (
          <button type="button" className="doctor__btn" data-primary disabled={chosen === 0} onClick={onFix}>
            {fixHeadline(chosen)}
          </button>
        )}
        {phase === "fixing" && (
          <button type="button" className="doctor__btn" data-primary aria-disabled="true">
            Fixing…
          </button>
        )}
        {offer?.kind === "header" && (
          <button type="button" className="doctor__btn" onClick={onUndo}>
            {offer.text}
          </button>
        )}
        {phase !== "fixing" && (
          <button type="button" className="doctor__btn" onClick={onRunAgain}>
            Run again
          </button>
        )}
      </>
    );

  return (
    <div className="doctor">
      <Page title="Doctor" subtitle={`Checked ${checked}`} actions={actions}>
        <div className="doctor__body" aria-busy={phase === "running" || phase === "fixing" ? "true" : undefined}>
          {phase === "running" && showProgress(elapsedMs) && <p className="doctor__progress">Checking Claude Code, Codex, Antigravity and Qwen Code…</p>}
          {phase === "failed" && <p className="doctor__failed">The doctor could not run: {failure}</p>}
          {clear && <EmptyState title={clearCopy.title} body={clearCopy.body} />}
          {listed && !clear && (
            <p className="doctor__summary">
              {c.issues} {plural(c.issues, "issue", "issues")}
              {phase === "fixed" ? (fixedCount >= c.issues ? ` · all fixed` : ` · ${fixedCount} fixed · ${c.issues - fixedCount} remain`) : c.fixable > 0 ? ` · ${c.fixable} can be fixed` : ""}
              {phase !== "fixed" && c.fixable === 0 && c.issues > 0 ? ` · Nothing to fix` : ""}
            </p>
          )}
          {offer?.kind === "stale" && (
            <p className="doctor__stale">
              {offer.text.replace(" Show differences.", "")}{" "}
              <button type="button" className="doctor__link" onClick={onShowDifferences}>
                Show differences
              </button>
            </p>
          )}
          {listed &&
            groups(report).map((g) => (
              <section key={g.group} className="doctor__group" aria-labelledby={`${uid}-${g.group}`}>
                <h2 id={`${uid}-${g.group}`} className="doctor__group-title">
                  {g.heading} <span className="doctor__group-count">{g.items.length}</span>
                </h2>
                <ul className="doctor__list">
                  {g.items.map((i) => (
                    <DoctorRow key={i.id} item={i} fixed={fixedIds.has(i.id)} checked={selected.has(i.id)} disabled={phase !== "ready"} onToggle={onToggle} onOpenAgents={onOpenAgents} />
                  ))}
                </ul>
              </section>
            ))}
        </div>
        {phase === "fixed" && undo && <UndoToast undo={undo} nowMs={nowMs} onUndo={onUndo} onRestart={onRestart ?? (() => {})} />}
      </Page>
    </div>
  );
}
