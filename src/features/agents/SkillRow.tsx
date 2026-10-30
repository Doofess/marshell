import { DiffView } from "../../components/DiffView/DiffView";
import type { AgentSetup } from "../../generated/AgentSetup";
import { previewDiff, type HookAgent } from "../../lib/api/agents";
import { removalBlock } from "./agentsState";
import type { SkillFlow } from "./useSkillFlow";

export type SkillRowHandlers = { install(): void; remove(): void; apply(): void; toggleDiff(): void; reload(): void; cancel(): void };

/** The skill's state as a line and the button for what the person can do about it. */
function stateOf(setup: AgentSetup): { line: string; label: string; act: "install" | "remove" } {
  switch (setup.skill) {
    case "none":
      return { line: "Not installed.", label: "Install the marshell skill", act: "install" };
    case "installed":
      return { line: `Installed in ${setup.skill_path}.`, label: "Remove the marshell skill", act: "remove" };
    case "outdated":
      return { line: "A newer version is ready.", label: "Update the marshell skill", act: "install" };
    case "edited":
      return { line: "Changed since Marshell wrote it. Updating shows what changes.", label: "Update the marshell skill", act: "install" };
  }
}

/** One agent's marshell skill (docs/PLAN.md slice 6h2): its state, and the preview, diff and Apply of an install, update or removal. */
export function SkillRow({ agent, setup, flow, on }: { agent: HookAgent; setup: AgentSetup; flow: SkillFlow | null; on: SkillRowHandlers }) {
  const { line, label, act } = stateOf(setup);
  const block = flow?.preview ? removalBlock(flow.preview) : undefined;
  return (
    <section className="agents-dialog__skill" aria-label="marshell skill" data-agent-skill={agent}>
      <p className="agents-dialog__line" dir="auto">
        {line}
      </p>
      {!flow && (
        <div className="agents-dialog__actions">
          <button type="button" className="agents-dialog__btn" onClick={on[act]}>
            {label}
          </button>
        </div>
      )}
      {flow && !flow.preview && !flow.error && <p className="agents-dialog__line">Checking what would change…</p>}
      {flow?.preview && (
        <>
          <p className="agents-dialog__line" dir="auto">
            {block ?? flow.preview.reason}
          </p>
          <div className="agents-dialog__actions">
            <button type="button" className="agents-dialog__btn" aria-expanded={flow.open} onClick={on.toggleDiff}>
              View exact changes
            </button>
          </div>
          {flow.open && <DiffView diff={previewDiff(flow.preview)} label="Exact changes to the marshell skill" />}
        </>
      )}
      {flow?.conflict && (
        <p className="agents-dialog__conflict">
          <strong>Review the difference.</strong> The file changed after this preview was made, so nothing was written. Reload to see what it says now.
        </p>
      )}
      {flow?.error && <p className="agents-dialog__error">{flow.error}</p>}
      {flow && (
        <div className="agents-dialog__actions">
          {flow.conflict ? (
            <button type="button" className="agents-dialog__btn" onClick={on.reload}>
              Reload
            </button>
          ) : (
            flow.preview && (
              <button type="button" className="agents-dialog__btn" disabled={flow.busy || !flow.preview.can_apply} onClick={on.apply}>
                Apply
              </button>
            )
          )}
          <button type="button" className="agents-dialog__btn" onClick={on.cancel}>
            Back
          </button>
        </div>
      )}
    </section>
  );
}
