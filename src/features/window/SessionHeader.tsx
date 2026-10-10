import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Chip } from "../../components/Chip/Chip";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { modeCaution } from "../sidebar/format";
import type { Mode, RowModel } from "../sidebar/types";
import "./SessionHeader.css";

const DASH = "–";
const MODE_LABELS: Record<Mode, string> = { manual: "Manual", plan: "Plan", "auto-edit": "Auto-edit", "full-auto": "Full auto", bypass: "Bypass" };

export const modeLabel = (mode?: Mode) => (mode ? MODE_LABELS[mode] : DASH);
/** docs/PLAN.md "Sidebar rows": the header is outlined red for bypass and auto. */
export const headerOutlined = (mode?: Mode) => mode === "bypass" || mode === "full-auto";

/**
 * The session header in the unified 40 px top bar: name, place, mode, model and effort, subagents, context, ports.
 * Unknown values are left out, never shown as a dash or guessed.
 */
export function SessionHeader({ row, ports = [] }: { row: RowModel; ports?: string[] }) {
  const caution = modeCaution(row.mode);
  const modelAndEffort = [row.model, row.effort].filter(Boolean).join(" · ");
  return (
    <div className="session-header" role="group" aria-label={`Session ${row.name}`} data-outlined={headerOutlined(row.mode) || undefined} data-agent={row.agent}>
      <AgentMark agent={row.agent} />
      {row.elevated && <AuxGlyph kind="elevated" label="Administrator" />}
      <span className="session-header__name" dir="auto">
        {row.name}
      </span>
      {row.elevated && <span className="session-header__label">Administrator</span>}
      <span className="session-header__place" dir="auto">
        {row.branch ? `${row.project} · ${row.branch}` : row.project}
      </span>
      {row.mode && <Chip tone={caution ? "caution" : "neutral"}>{modeLabel(row.mode)}</Chip>}
      {modelAndEffort && <span className="session-header__meta">{modelAndEffort}</span>}
      {row.subagents !== undefined && <span className="session-header__meta">{row.subagents} subagents</span>}
      <span className="session-header__spacer" />
      <ContextRing pct={row.contextPct} />
      {ports.map((p) => (
        <Chip key={p}>{p}</Chip>
      ))}
    </div>
  );
}
