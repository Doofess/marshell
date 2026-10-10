import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import { EMPTY_STATES } from "../onboarding/emptyStates";
import { DoctorRow } from "./DoctorRow";
import { UndoToast } from "./UndoToast";
import { showProgress } from "./doctorUi";
import { counts, fixHeadline, restartSessionsOf } from "./report";
import type { DoctorReport } from "./types";
import { undoOffer, type UndoState } from "./undo";
import "./Doctor.css";

export type DoctorPhase = "running" | "ready" | "fixing" | "fixed" | "failed";
const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

/**
 * The Doctor page: one report, one combined fix. Nothing is drawn for the first 150 ms of a check; a failure says why and
 * offers Run again; after a fix each fixed row says Fixed in words, the receipt toast floats for 10 seconds and then Undo
 * last fix stays in the header. Information (managed settings) is listed and never has a control.
 */
export function DoctorPage({ report, phase, undo, nowMs, elapsedMs = 0, failure }: { report: DoctorReport; phase: DoctorPhase; undo?: UndoState; nowMs?: number; elapsedMs?: number; failure?: string }) {
  const c = counts(report);
  const now = nowMs ?? report.checkedAtMs;
  const offer = phase === "fixed" && undo ? undoOffer(undo, now) : undefined;
  const clear = report.items.length === 0 && phase === "ready";
  const clearCopy = EMPTY_STATES.find((e) => e.id === "doctor-clear")!;
  const checked = nowMs === undefined ? "–" : `${Math.max(0, Math.round((nowMs - report.checkedAtMs) / 1000))} s ago`;
  const restart = restartSessionsOf(report.items.filter((i) => i.fixable));
  const listed = phase !== "running" && phase !== "failed";

  const actions =
    phase === "running" ? null : (
      <>
        {phase === "ready" && c.fixable > 0 && (
          <button type="button" className="doctor__btn" data-primary>
            {fixHeadline(report)}
          </button>
        )}
        {phase === "fixing" && (
          <button type="button" className="doctor__btn" data-primary aria-disabled="true">
            Fixing…
          </button>
        )}
        {offer?.kind === "header" && (
          <button type="button" className="doctor__btn">
            {offer.text}
          </button>
        )}
        {phase !== "fixing" && (
          <button type="button" className="doctor__btn">
            Run again
          </button>
        )}
      </>
    );

  return (
    <div className="doctor">
      <Page title="Doctor" subtitle={`Checked ${checked}`} actions={actions}>
        <div className="doctor__body" aria-busy={phase === "running" || phase === "fixing" ? "true" : undefined}>
          {phase === "running" && showProgress(elapsedMs) && <p className="doctor__progress">Checking Claude Code, Codex and Gemini…</p>}
          {phase === "failed" && <p className="doctor__failed">The doctor could not run: {failure}</p>}
          {clear && <EmptyState title={clearCopy.title} body={clearCopy.body} />}
          {listed && !clear && (
            <p className="doctor__summary">
              {c.issues} {plural(c.issues, "issue", "issues")}
              {c.fixable > 0 ? ` · ${c.fixable} can be fixed` : ""}
              {c.fixable === 0 && c.issues > 0 ? ` · ${fixHeadline(report)}` : ""}
            </p>
          )}
          {offer?.kind === "stale" && (
            <p className="doctor__stale">
              {offer.text.replace(" Show differences.", "")}{" "}
              <button type="button" className="doctor__link">
                Show differences
              </button>
            </p>
          )}
          {listed && report.items.length > 0 && (
            <ul className="doctor__list">
              {report.items.map((i) => (
                <DoctorRow key={i.id} item={i} fixed={phase === "fixed"} />
              ))}
            </ul>
          )}
        </div>
        {phase === "fixed" && undo && <UndoToast undo={undo} nowMs={now} restart={restart} />}
      </Page>
    </div>
  );
}
