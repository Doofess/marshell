import { AgentMark } from "../../components/AgentMark/AgentMark";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { CLI_NAME, type Detection } from "./consent";
import "./Onboarding.css";

function stateSentence(d: Detection): string | null {
  if (d.configState === "unparsable") return `Its settings file does not parse (line ${d.parseError?.line ?? 0}), so Marshell will not edit it.`;
  if (d.configState === "read-only") return "Its settings file is read-only, so Marshell cannot add hooks to it.";
  return null;
}

/** Screen 1: what Marshell found. One row per CLI, with the whole config path (a folder with a space is shown whole), and a way forward when nothing is installed. */
export function Detected({ detections }: { detections: Detection[] }) {
  if (detections.every((d) => !d.installed)) {
    return <EmptyState title="No coding agents found" body="Install Claude Code, Codex or Gemini, or continue with shells and tools." action={{ label: "Continue with shells and tools" }} />;
  }
  return (
    <ul className="detected" aria-label="Coding agents found">
      {detections.map((d) => {
        const problem = d.installed ? stateSentence(d) : null;
        return (
          <li key={d.cli} className="detected__row" data-agent={d.cli} data-installed={d.installed}>
            <AgentMark agent={d.cli} size={20} />
            <div className="detected__text">
              <p className="detected__name">
                <span dir="auto">{CLI_NAME[d.cli]}</span>
                <span className="detected__version">{d.installed ? `version ${d.version ?? "–"}` : "Not installed"}</span>
              </p>
              {d.installed && (
                <p className="detected__path">
                  <code dir="ltr">{d.configPath}</code>
                </p>
              )}
              {problem && <p className="detected__problem">{problem}</p>}
              {d.installed && d.managed && (
                <p className="detected__problem">
                  <code dir="ltr">{d.managed.path}</code> is managed by your organisation. Marshell lists it and never changes it.
                </p>
              )}
            </div>
          </li>
        );
      })}
    </ul>
  );
}
