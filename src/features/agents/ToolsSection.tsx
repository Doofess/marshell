import { useCallback, useEffect, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import type { OwnToolProblem } from "../../generated/OwnToolProblem";
import type { ToolCatalog } from "../../generated/ToolCatalog";
import { toolsApi } from "../../lib/api/tools";
import "./OwnAgentsSection.css";

/** A problem of the file itself: it could not be read, or an entry had no id to name (the core gives those index 0). */
const isFileProblem = (p: OwnToolProblem) => p.id === null && p.index === 0;

/** The "Tools" section of the Agents dialog: the shipped and own tool profiles with their presets. Presentational. */
export function ToolsView({ catalog }: { catalog: ToolCatalog }) {
  const file = catalog.problems.filter(isFileProblem);
  const rest = catalog.problems.filter((p) => !isFileProblem(p));
  const listed = new Set(catalog.tools.map((t) => t.id));
  // A refused entry is not a tool, so most problems have no entry to sit under; they are said by their place in the file.
  const under = (id: string) => rest.filter((p) => p.id === id);
  const unlisted = rest.filter((p) => p.id === null || !listed.has(p.id));
  return (
    <section className="own-agents" aria-label="Tools">
      <h3 className="own-agents__title">Tools</h3>
      {file.length > 0 && (
        <ul className="own-agents__problems">
          {file.map((p) => (
            <li key={p.reason} className="own-agents__problem" dir="auto">
              {p.reason}
            </li>
          ))}
        </ul>
      )}
      <ul className="own-agents__list">
        {catalog.tools
          .filter((t) => t.presets.length > 0)
          .map((t) => (
            <li key={t.id} className="own-agents__item">
              <div className="own-agents__summary">
                <span className="own-agents__name" dir="auto">
                  {t.name}
                </span>
                <span className="own-agents__command">{t.own ? "Own" : "Shipped"}</span>
                <span className="own-agents__command" dir="auto">
                  {t.presets.map((p) => p.name).join(", ")}
                </span>
                {under(t.id).map((p) => (
                  <span key={`${p.index}-${p.reason}`} className="own-agents__problem" dir="auto">
                    {p.reason}
                  </span>
                ))}
              </div>
            </li>
          ))}
      </ul>
      {unlisted.length > 0 && (
        <ul className="own-agents__problems">
          {unlisted.map((p) => (
            <li key={`${p.index}-${p.reason}`} className="own-agents__problem" dir="auto">
              {p.id === null ? `Entry ${p.index + 1}` : `${p.id} (entry ${p.index + 1})`}: {p.reason}
            </li>
          ))}
        </ul>
      )}
      <p className="own-agents__line">Add your own in tools.json in the Marshell folder. Settings gets an editor later.</p>
    </section>
  );
}

/** The Tools section on the core's data, read when the dialog opens. A failed read says so and offers Try again. */
export function ToolsSection({ endpoint }: { endpoint: Endpoint }) {
  const [catalog, setCatalog] = useState<ToolCatalog | null>(null);
  const [failed, setFailed] = useState(false);
  const load = useCallback(() => {
    setFailed(false);
    toolsApi.get(endpoint).then(setCatalog, () => setFailed(true));
  }, [endpoint]);
  useEffect(() => {
    load();
  }, [load]);
  if (failed) {
    return (
      <section className="own-agents" aria-label="Tools">
        <h3 className="own-agents__title">Tools</h3>
        <p className="own-agents__problem">
          Couldn&apos;t read the tools list.
        </p>
        <div className="own-agents__actions">
          <button type="button" className="own-agents__btn" onClick={load}>
            Try again
          </button>
        </div>
      </section>
    );
  }
  if (catalog === null) {
    return (
      <section className="own-agents" aria-label="Tools">
        <h3 className="own-agents__title">Tools</h3>
        <p className="own-agents__line">Reading the tools…</p>
      </section>
    );
  }
  return <ToolsView catalog={catalog} />;
}
