import { useId } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { DiffView } from "../../components/DiffView/DiffView";
import { Switch } from "../../components/Switch/Switch";
import { canEdit, CAPABILITIES, CLI_NAME, consentHeadline, type Capability, type ConsentPlan, type Detection } from "./consent";
import "./Onboarding.css";

const CAP_LABEL: Record<Capability, string> = { status: "Show status and context", approve: "Approve from anywhere", statusline: "Keep my status line working" };

/** Screen 2: one card per CLI, in plain words: what changes and where, the exact diff on request, the backup and the way out. A file that cannot be edited gets the reason instead of controls. */
export function ConsentCard({ scenario, on, open = false }: { scenario: { detection: Detection; plan: ConsentPlan }; on: Record<Capability, boolean>; open?: boolean }) {
  const { detection: d, plan } = scenario;
  const titleId = useId();
  const edit = canEdit(d);
  const name = CLI_NAME[d.cli];
  return (
    <article className="consent" data-agent={d.cli} data-blocked={edit.ok ? undefined : ""} aria-labelledby={titleId}>
      <header className="consent__head">
        <AgentMark agent={d.cli} size={20} />
        <h3 id={titleId} className="consent__name">
          {name}
        </h3>
      </header>
      <p className="consent__what" dir="auto">
        {plan.headline ?? consentHeadline(d, on, plan.handOver)}
      </p>
      {edit.ok ? (
        <>
          <ul className="consent__switches">
            {(["status", "approve", "statusline"] as const).map((c) => (
              <li key={c}>
                <Switch label={CAP_LABEL[c]} checked={on[c] && CAPABILITIES[d.cli][c].available} disabled={!CAPABILITIES[d.cli][c].available} reason={CAPABILITIES[d.cli][c].reason} />
              </li>
            ))}
            {d.skipsAll && (
              <li>
                <Switch label="Let Marshell set the mode" checked={plan.handOver ?? false} />
              </li>
            )}
          </ul>
          {d.skipsAll && (
            <p className="consent__handover">
              {name} is set to skip every permission prompt (<code>{d.skipsAll.setting}</code>), so Marshell can&apos;t ask you about anything. Turning it off lets each
              session&apos;s mode decide; Bypass keeps today&apos;s behaviour.
            </p>
          )}
          <button type="button" className="consent__toggle" aria-expanded={open}>
            View exact changes
          </button>
          {open && <DiffView diff={plan.diff} label={`Exact changes to ${d.hooks ? `${d.hooks.path} and ${d.configPath}` : d.configPath}`} />}
          {plan.chainPath && on.statusline && CAPABILITIES[d.cli].statusline.available && (
            <p className="consent__chain">
              Your own status line command is saved in <code>{plan.chainPath}</code> and keeps running: Marshell&apos;s runs first and then yours.
            </p>
          )}
          <p className="consent__meta">
            A backup is saved first, in <code>{plan.backupPath}</code>.
            {d.hooks && !d.hooks.exists && (
              <>
                {" "}
                <code>{d.hooks.path.split("\\").pop()}</code> is a new file, so it needs no backup.
              </>
            )}{" "}
            To undo: {plan.removal}
          </p>
          {plan.needsTrust && (
            <>
              <p className="consent__trust">
                Codex runs hooks only after you trust them. After you allow this, Marshell asks you to open <code>/hooks</code> and trust them there.
              </p>
              <p className="consent__source">
                The hook entries go in <code>config.toml</code> only, not in <code>hooks.json</code> as well: Codex reads both and would warn about duplicates.
              </p>
            </>
          )}
        </>
      ) : (
        <p className="consent__why">{edit.reason} Marshell leaves it alone. Fix the file and run the doctor, or skip.</p>
      )}
      {d.managed && (
        <p className="consent__managed">
          <code>{d.managed.path}</code> is managed by your organisation. Marshell can&apos;t change it.
        </p>
      )}
      <footer className="consent__foot">
        <button type="button" className="consent__skip">
          Skip: use limited status
        </button>
      </footer>
    </article>
  );
}
