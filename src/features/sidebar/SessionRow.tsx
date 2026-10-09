import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { formatDuration, formatUsage, modeCaution } from "./format";
import { rowLabel, rowTitle } from "./labels";
import { displayWidth, splitForMiddle } from "./truncate";
import type { Density, RowModel } from "./types";
import "./SessionRow.css";

const NEEDS_YOU = new Set(["needs-permission", "needs-question"]);

/** One sidebar row (docs/PLAN.md "Sidebar rows"). Height is fixed per density; hover never changes it. */
export function SessionRow({ row, density, selected = false }: { row: RowModel; density: Density; selected?: boolean }) {
  const caution = modeCaution(row.mode);
  const time = row.waitingMs ?? (row.status === "done-unseen" || row.status === "done-seen" || row.status === "ended" ? row.ageMs : undefined);
  const branch = row.branch ? splitForMiddle(row.branch) : null;

  return (
    <div
      className="session-row"
      role="listitem"
      tabIndex={0}
      title={rowTitle(row)}
      aria-label={rowLabel(row)}
      aria-current={selected || undefined}
      data-density={density}
      data-agent={row.agent}
      data-status={row.status}
      data-needs-you={NEEDS_YOU.has(row.status) || undefined}
      data-unseen={row.status === "done-unseen" || undefined}
      data-ended={row.status === "ended" || undefined}
      data-flash={row.flash || undefined}
    >
      <span className="session-row__stripe" aria-hidden="true" />
      <div className="session-row__line1" aria-hidden="true">
        <StatusGlyph kind={row.status} />
        {row.elevated && <AuxGlyph kind="elevated" />}
        {/* Names up to 8 columns wide never shrink; longer ones keep at least 8 (docs/PLAN.md truncation order). */}
        <span className="session-row__name" data-short={displayWidth(row.name) <= 8 || undefined}>
          {row.name}
        </span>
        {row.status === "done-unseen" && <span className="session-row__unread" />}
        <span className="session-row__project">{row.project}</span>
        {branch && (
          <>
            <span className="session-row__sep">·</span>
            <span className="session-row__branch-head">{branch.head}</span>
            <span className="session-row__branch-tail">{branch.tail}</span>
          </>
        )}
        <span className="session-row__cluster">
          {caution && <AuxGlyph kind="caution" label={caution} />}
          {row.muted && <AuxGlyph kind="muted" />}
          {time !== undefined && <span className="session-row__time">{formatDuration(time)}</span>}
          <ContextRing pct={row.contextPct} />
        </span>
      </div>
      {density !== "compact" && (
        <div className="session-row__phrase" aria-hidden="true">
          {row.phrase}
          {row.muted && <span className="session-row__suffix"> · muted</span>}
        </div>
      )}
      {density === "expanded" && (
        <div className="session-row__details" aria-hidden="true">
          <div className="session-row__meta">
            <span>{row.model ?? "–"}</span>
            <span>{row.effort ?? "–"}</span>
            {row.mode === "plan" && <span className="session-row__pill">Plan</span>}
            {caution && <span className="session-row__caution-text">{caution}</span>}
            <span>{row.subagents !== undefined ? `${row.subagents} subagents` : "–"}</span>
          </div>
          {row.recap && (
            <p className="session-row__recap">
              {row.recap.text} <span className="session-row__age">{formatDuration(row.recap.ageMs)} ago</span>
            </p>
          )}
          <div className="session-row__usage">{formatUsage(row.usage)}</div>
        </div>
      )}
    </div>
  );
}
