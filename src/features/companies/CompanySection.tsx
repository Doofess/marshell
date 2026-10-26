import { useEffect, useState } from "react";
import type { AgentInstall } from "../../generated/AgentInstall";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import type { Endpoint } from "../../generated/Endpoint";
import type { SettingValue } from "../../generated/SettingValue";
import { getInstall, getSettingsView } from "../../lib/api/accounts";
import { RawEditor } from "../settings/RawEditor";
import { ScopeBadge } from "../settings/ScopeBadge";
import type { CompanyAgent } from "../settings/settingsSections";
import { COMPANY_FIELDS } from "./companyFields";
import { fileLine, MANUAL_UPDATE, newerLine, runningNote, versionLine } from "./companyText";
import "../settings/Settings.css";
import "./CompanySection.css";

export type CompanyMode = "form" | "raw";

export type CompanyViewProps = {
  agent: CompanyAgent;
  name: string;
  install: AgentInstall | null;
  view: AgentSettingsView | null;
  running: number;
  mode: CompanyMode;
  onMode: (mode: CompanyMode) => void;
  onUpdate: () => void;
};

const shown = (v: SettingValue | undefined) => v?.value ?? "–";

/** The manual-update sentence with its commands in `<code>`. */
function ManualUpdate({ agent, name }: { agent: CompanyAgent; name: string }) {
  const m = MANUAL_UPDATE[agent];
  return (
    <p className="company__line">
      {`Marshell can't tell how ${name} was installed. Update it with `}
      <code>{m.update}</code>
      {m.npm && (
        <>
          {", or "}
          <code>{m.npm}</code>
          {" for an npm install"}
        </>
      )}
      {"."}
    </p>
  );
}

/** One company CLI, read-only: version and Update, then its user settings file as a form or as text. Presentational. */
export function CompanyView({ agent, name, install, view, running, mode, onMode, onUpdate }: CompanyViewProps) {
  const newer = install ? newerLine(install) : null;
  const note = runningNote(name, running);
  const file = view ? fileLine(view) : null;
  const byKey = new Map((view?.curated ?? []).map((v) => [v.key, v]));
  return (
    <div className="company">
      {!install && !view && <p className="company__line">{`Reading ${name}…`}</p>}
      {install && (
        <div className="company__version">
          <p className="company__line company__line--strong">{versionLine(install)}</p>
          {newer && <p className="company__line">{newer}</p>}
          {install.update_command ? (
            <button type="button" className="settings__btn company__update" onClick={onUpdate}>
              {`Update ${name}`}
            </button>
          ) : (
            <ManualUpdate agent={agent} name={name} />
          )}
          {install.update_command && note && <p className="company__line">{note}</p>}
        </div>
      )}
      {view && (
        <>
          <p className="company__path" dir="auto">
            {view.path}
          </p>
          {file && <p className="company__line">{file}</p>}
          <div className="company__modes" role="radiogroup" aria-label="View">
            <label className="company__mode">
              <input type="radio" name={`company-view-${agent}`} value="form" checked={mode === "form"} onChange={() => onMode("form")} />
              <span>Form</span>
            </label>
            <label className="company__mode" data-disabled={view.raw === null || undefined}>
              <input type="radio" name={`company-view-${agent}`} value="raw" checked={mode === "raw"} disabled={view.raw === null} onChange={() => onMode("raw")} />
              <span>Raw file</span>
            </label>
          </div>
          {mode === "raw" && view.raw !== null ? (
            <div className="company__raw">
              {agent === "codex" ? <textarea className="raw-editor__text" aria-label={view.path} rows={Math.min(24, Math.max(6, view.raw.split("\n").length))} spellCheck={false} defaultValue={view.raw} readOnly /> : <RawEditor path={view.path} text={view.raw} readOnly />}
              <p className="company__note">Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden.</p>
            </div>
          ) : (
            <>
              <ul className="settings__fields">
                {COMPANY_FIELDS[agent].map((f) => {
                  const v = byKey.get(f.key);
                  return (
                    <li key={f.key} className="field">
                      <div className="field__head">
                        <span className="field__label">{f.label}</span>
                        <ScopeBadge scope={v?.scope ?? "user"} />
                      </div>
                      <p className="field__help" dir="auto">
                        {f.help}
                      </p>
                      <p className="company__value" dir="auto">
                        {shown(v)}
                      </p>
                    </li>
                  );
                })}
              </ul>
              {view.others.length > 0 && (
                <>
                  <h3 className="company__others">Other settings in this file</h3>
                  <ul className="settings__fields">
                    {view.others.map((o) => (
                      <li key={o.key} className="field">
                        <div className="field__head">
                          <code className="company__key">{o.key}</code>
                          <ScopeBadge scope={o.scope} />
                        </div>
                        <p className="company__value" dir="auto">
                          {shown(o)}
                        </p>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </>
          )}
        </>
      )}
    </div>
  );
}

/** Fetches the CLI's install and settings view when it opens and again, with a fresh version read, whenever `refresh` changes. */
export function CompanySection({ endpoint, agent, name, running, onUpdate, refresh }: { endpoint: Endpoint; agent: CompanyAgent; name: string; running: number; onUpdate: (agent: CompanyAgent) => void; refresh: number }) {
  const [install, setInstall] = useState<AgentInstall | null>(null);
  const [view, setView] = useState<AgentSettingsView | null>(null);
  const [failed, setFailed] = useState(false);
  const [mode, setMode] = useState<CompanyMode>("form");
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const [i, v] = await Promise.all([getInstall(endpoint, agent, refresh > 0), getSettingsView(endpoint, agent)]);
        if (!live) return;
        setInstall(i);
        setView(v);
        setFailed(false);
      } catch {
        if (live) setFailed(true);
      }
    })();
    return () => {
      live = false;
    };
  }, [endpoint, agent, refresh]);
  return (
    <>
      {failed && <p className="company__error">{`Couldn't read ${name} from Marshell's core. Close Settings and try again.`}</p>}
      {(!failed || install || view) && <CompanyView agent={agent} name={name} install={install} view={view} running={running} mode={view?.raw === null ? "form" : mode} onMode={setMode} onUpdate={() => onUpdate(agent)} />}
    </>
  );
}
