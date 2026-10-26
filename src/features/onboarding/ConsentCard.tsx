import { useId } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { DiffView } from "../../components/DiffView/DiffView";
import { Switch } from "../../components/Switch/Switch";
import { canEdit, CAPABILITIES, CLI_NAME, consentHeadline, fileName, folderOf, type Capability, type ConsentPlan, type Detection } from "./consent";
import "./Onboarding.css";

const CAP_LABEL: Record<Capability, string> = { status: "Show status and context", approve: "Approve from anywhere", statusline: "Keep my status line working" };

const ALL: Capability[] = ["status", "approve", "statusline"];

/** Screen 2: one card per CLI, in plain words: what changes and where, the exact diff on request, the backup and the way out. A file that cannot be edited gets the reason instead of controls. */
export function ConsentCard({
  scenario,
  on,
  open = false,
  offer = ALL,
  busy = false,
  onApply,
  onSkip,
  skipLabel = "Skip: use limited status",
  error,
  conflict,
  onToggleDiff,
  onSwitch,
}: {
  scenario: { detection: Detection; plan: ConsentPlan };
  on: Record<Capability, boolean>;
  open?: boolean;
  /** The switches shown. A user with no status line of their own is not offered "Keep my status line working" whatever this says. */
  offer?: Capability[];
  busy?: boolean;
  onApply?: () => void;
  onSkip?: () => void;
  /** The Skip button's words; "Done" when there is nothing left to change. */
  skipLabel?: string;
  error?: string;
  /** The file changed since the preview: the card offers a reload, never an overwrite. */
  conflict?: { onReload: () => void };
  /** "View exact changes" opens or closes the diff. Left out (stories), the button does nothing and `open` decides. */
  onToggleDiff?: () => void;
  /** With it, the offered switches are controlled by `on` and a change calls this. Left out (stories), they keep their own state. */
  onSwitch?: (capability: Capability, checked: boolean) => void;
}) {
  const { detection: d, plan } = scenario;
  const titleId = useId();
  const edit = canEdit(d);
  const name = CLI_NAME[d.cli];
  const change = (c: Capability, checked: boolean) => {
    // An upgrade offers only approval: its live status hooks are already installed.
    if (offer.includes("status")) {
      if (c === "approve" && checked && !on.status) onSwitch?.("status", true);
      if (c === "status" && !checked && on.approve) onSwitch?.("approve", false);
    }
    onSwitch?.(c, checked);
  };
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
            {offer.filter((c) => c !== "statusline" || d.hasOwnStatusLine !== false).map((c) => (
              <li key={c}>
                <Switch label={CAP_LABEL[c]} checked={on[c] && CAPABILITIES[d.cli][c].available} disabled={!CAPABILITIES[d.cli][c].available} reason={CAPABILITIES[d.cli][c].reason} onChange={onSwitch && ((v) => change(c, v))} />
              </li>
            ))}
            {d.skipsAll && (
              <li>
                <Switch label="Let Marshell set the mode" checked={plan.handOver ?? false} />
              </li>
            )}
          </ul>
          {offer.includes("approve") && offer.includes("status") && <p className="consent__meta">Approve from anywhere needs live status to know when a request is answered.</p>}
          {d.skipsAll && (
            <p className="consent__handover">
              {name} is set to skip every permission prompt (<code>{d.skipsAll.setting}</code>), so Marshell can&apos;t ask you about anything. Turning it off lets each
              session&apos;s mode decide; Bypass keeps today&apos;s behaviour.
            </p>
          )}
          <button type="button" className="consent__toggle" aria-expanded={open} onClick={onToggleDiff}>
            View exact changes
          </button>
          {open &&
            plan.deleted?.map((path) => (
              <p key={path} className="consent__meta">
                <code>{fileName(path)}</code> will be deleted
              </p>
            ))}
          {open && <DiffView diff={plan.diff} label={`Exact changes to ${d.hooks ? `${d.hooks.path} and ${d.configPath}` : d.configPath}`} />}
          {plan.chainPath && on.statusline && d.hasOwnStatusLine !== false && CAPABILITIES[d.cli].statusline.available && (
            <p className="consent__chain">
              Your own status line command is saved in <code>{plan.chainPath}</code> and keeps running: Marshell&apos;s runs first and then yours.
            </p>
          )}
          <p className="consent__meta">
            {d.hooks?.exists ? (
              <>
                Backups of both files are saved first, in <code>{folderOf(plan.backupPath)}</code>.
              </>
            ) : (
              <>
                A backup is saved first, in <code>{plan.backupPath}</code>.
              </>
            )}
            {d.hooks && !d.hooks.exists && (
              <>
                {" "}
                <code>{fileName(d.hooks.path)}</code> is a new file, so it needs no backup.
              </>
            )}{" "}
            To undo: {plan.removal}
          </p>
          {plan.needsTrust && (
            <>
              <p className="consent__trust">
                Codex runs hooks only after you trust them. The next time Codex starts, it lists them as new: choose <strong>Trust all and continue</strong>, or review them first in <code>/hooks</code>.
              </p>
              <p className="consent__source">
                The hook entries go in <code>hooks.json</code> only, not in <code>config.toml</code> as well: Codex reads both and would warn about duplicates.
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
      {conflict && (
        <p className="consent__conflict">
          <strong>Review the difference.</strong> The file changed after this preview was made, so nothing was written. Reload to see what it says now.
        </p>
      )}
      {error && (
        <p className="consent__error" role="alert">
          {error}
        </p>
      )}
      <footer className="consent__foot">
        {conflict ? (
          <button type="button" className="consent__toggle" onClick={conflict.onReload}>
            Reload
          </button>
        ) : (
          edit.ok &&
          onApply && (
            <button type="button" className="consent__apply" disabled={busy} onClick={onApply}>
              Allow
            </button>
          )
        )}
        <button type="button" className="consent__skip" onClick={onSkip}>
          {skipLabel}
        </button>
      </footer>
    </article>
  );
}
