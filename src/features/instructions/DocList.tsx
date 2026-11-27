import { useId } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import type { CopyRow } from "../../generated/CopyRow";
import type { DocScope } from "../../generated/DocScope";
import type { DocView } from "../../generated/DocView";
import type { LibraryView } from "../../generated/LibraryView";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { LoadsIn } from "./LoadsIn";

export type OpenDoc = { scope: DocScope; name: string };

function Docs({ docs, scope, open, onOpen }: { docs: DocView[]; scope: DocScope; open: OpenDoc | null; onOpen: (scope: DocScope, name: string) => void }) {
  if (docs.length === 0) return <p className="doclist__empty">No docs yet.</p>;
  return (
    <ul className="doclist__docs">
      {docs.map(({ doc }) => {
        const current = open !== null && open.scope === scope && open.name.toLowerCase() === doc.name.toLowerCase();
        return (
          <li key={doc.name}>
            <button type="button" {...selectedProps(current, "current", "doclist__doc")} onClick={() => onOpen(scope, doc.name)}>
              {doc.name}
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function Orphans({ rows }: { rows: CopyRow[] }) {
  const id = useId();
  if (rows.length === 0) return null;
  return (
    <section className="doclist__orphans" aria-labelledby={id}>
      <h4 id={id} className="doclist__sub">
        Copies with no doc
      </h4>
      <LoadsIn rows={rows} orphan />
    </section>
  );
}

/**
 * The page's left column: the docs for every project, then the picked project's docs. A project that can't be read
 * says why and offers no new doc; with no project the sentence says how to get one.
 */
export function DocList({ view, projects, project, open, onPickProject, onOpen, onNew }: { view: LibraryView | null; projects: ProjectInfo[]; project: string | null; open: OpenDoc | null; onPickProject: (path: string) => void; onOpen: (scope: DocScope, name: string) => void; onNew: (scope: DocScope) => void }) {
  const userId = useId();
  const pick = useId();
  const noProject = project === null && projects.length === 0;
  return (
    <nav className="doclist" aria-label="Instructions">
      <section className="doclist__group" aria-labelledby={userId}>
        <h3 id={userId} className="doclist__title">
          Every project
        </h3>
        <Docs docs={view?.user_docs ?? []} scope="user" open={open} onOpen={onOpen} />
        <button type="button" className="instructions__btn" onClick={() => onNew("user")}>
          New doc for every project
        </button>
        <Orphans rows={view?.user_orphans ?? []} />
      </section>
      <section className="doclist__group" aria-label="This project">
        {noProject ? (
          <p className="doclist__empty">No project yet. Open a session in a folder, or add one in Settings → Projects.</p>
        ) : (
          <>
            <select id={pick} className="instructions__select" aria-label="Project" value={project ?? ""} onChange={(e) => onPickProject(e.target.value)}>
              {project === null && (
                <option value="" disabled>
                  Choose a project
                </option>
              )}
              {projects.map((p) => (
                <option key={p.path} value={p.path}>
                  {p.name}
                </option>
              ))}
            </select>
            {project !== null && view?.project_note !== undefined && <p className="doclist__note">{view.project_note}</p>}
            {project !== null && view?.project_note === undefined && (
              <>
                <Docs docs={view?.project_docs ?? []} scope="project" open={open} onOpen={onOpen} />
                <button type="button" className="instructions__btn" onClick={() => onNew("project")}>
                  New doc for this project
                </button>
              </>
            )}
            {project !== null && <Orphans rows={view?.project_orphans ?? []} />}
          </>
        )}
      </section>
    </nav>
  );
}
