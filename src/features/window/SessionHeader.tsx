import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Chip } from "../../components/Chip/Chip";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import { sessionWord, SESSION_NUMBER_HINT } from "../sidebar/sessionNumber";
import { AuxGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { modeCaution } from "../sidebar/format";
import type { Mode, RowModel } from "../sidebar/types";
import "./SessionHeader.css";

const DASH = "–";
const MODE_LABELS: Record<Mode, string> = { manual: "Manual", plan: "Plan", "auto-edit": "Auto-edit", "full-auto": "Full auto", bypass: "Bypass", "dont-ask": "Don't ask" };

export const modeLabel = (mode?: Mode) => (mode ? MODE_LABELS[mode] : DASH);
/** docs/PLAN.md "Sidebar rows": the header is marked red for bypass and auto, with a tint and a start-edge bar (never a box). */
/** What the mode chip says: the mode a session started in is all Marshell knows until the CLI reports changes. */
const startedIn = (mode: Mode) => `Started in ${modeLabel(mode)}. A change made inside the CLI isn't shown yet.`;

export const headerDanger = (mode?: Mode) => mode === "bypass" || mode === "full-auto";

/**
 * The session header in the unified 40 px top bar: name, place, mode, model and effort, subagents, context, ports.
 * Unknown values are left out, never shown as a dash or guessed.
 */
export function SessionHeader({ row, ports = [], onMode }: { row: RowModel; ports?: string[]; /** Makes the mode chip a button; it gives the chip's corner, where the mode menu opens. */ onMode?: (anchor: { x: number; y: number }) => void }) {
  const caution = modeCaution(row.mode);
  const modelAndEffort = [row.model, row.effort].filter(Boolean).join(" · ");
  return (
    <div className="session-header" role="group" aria-label={`${sessionWord(row.number)}, ${row.name}`} data-danger={headerDanger(row.mode) || undefined} data-agent={row.agent}>
      <AgentMark agent={row.agent} />
      {row.elevated && <AuxGlyph kind="elevated" label="Administrator" />}
      <SessionNumber n={row.number} title={SESSION_NUMBER_HINT} />
      <span className="session-header__name" dir="auto">
        {row.name}
      </span>
      {row.elevated && <span className="session-header__label">Administrator</span>}
      <span className="session-header__place" dir="auto">
        {row.branch ? `${row.project} · ${row.branch}` : row.project}
      </span>
      {row.mode && onMode && (
        // A button, so its label is announced (a bare span's aria-label may not be) and a keyboard reaches the mode menu.
        <button
          type="button"
          className="session-header__mode"
          aria-label={startedIn(row.mode)}
          title={startedIn(row.mode)}
          aria-haspopup="menu"
          onClick={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            onMode({ x: r.left, y: r.bottom });
          }}
        >
          <Chip tone={caution ? "caution" : "neutral"}>{modeLabel(row.mode)}</Chip>
        </button>
      )}
      {row.mode && !(onMode) && (
        <Chip tone={caution ? "caution" : "neutral"} title={startedIn(row.mode)} label={startedIn(row.mode)}>
          {modeLabel(row.mode)}
        </Chip>
      )}
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
