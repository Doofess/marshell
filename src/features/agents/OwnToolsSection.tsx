import { useCallback, useEffect, useRef, useState, type Ref } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import type { OwnToolProblem } from "../../generated/OwnToolProblem";
import type { OwnTools } from "../../generated/OwnTools";
import type { PromptKind } from "../../generated/PromptKind";
import { ownTools } from "../../lib/api/catalog";
import { focusAfterCancel, focusAfterRemove, type FormFocus } from "./ownAgents";
import { blankDraft, blankPattern, blankPreset, buildSave, CONFLICT_TEXT, draftOf, escapeCancels, outcomeOf, placeholders, profileOf, refusalOf, removedText, savedText, slug, type Change, type ChangeKind, type ToolDraft } from "./ownTools";
import "./OwnAgentsSection.css";
import "./OwnToolsSection.css";

/** The open form: a new tool (`index` null) or one being edited, with the core's reason when it refused the entry. */
export type Editing = { index: number | null; draft: ToolDraft; busy: boolean; problem?: string };

export const ERROR_ID = "own-tool-error";

export type OwnToolsViewProps = {
  /** null until the file has been read. */
  own: OwnTools | null;
  /** Problems to show on entries instead of the file's own, such as a 422's. */
  problems?: OwnToolProblem[];
  editing: Editing | null;
  /** The index of the entry the "Remove <name>?" question is about. */
  removing: number | null;
  conflict?: boolean;
  message?: string | null;
  sectionRef?: Ref<HTMLElement>;
  onAdd: () => void;
  onEdit: (index: number) => void;
  onRemove: (index: number) => void;
  onConfirmRemove: () => void;
  onCancelRemove: () => void;
  onChange: (draft: ToolDraft) => void;
  onSave: () => void;
  onCancel: () => void;
  onReload: () => void;
};

/** The own tools of `tools.json`: the list, and the form to add or edit one. Presentational. */
export function OwnToolsView({ own, problems: shown, editing, removing, conflict, message, sectionRef, ...on }: OwnToolsViewProps) {
  const tools = own ? own.tools : [];
  const problems = shown ?? (own ? own.problems : []);
  // A problem at an index with no entry belongs to the file (it couldn't be read).
  const file = problems.filter((p) => p.index >= tools.length);
  return (
    <section ref={sectionRef} className="own-agents" aria-label="Your own tools">
      <h3 className="own-agents__title">Your own tools</h3>
      {file.length > 0 && (
        <ul className="own-agents__problems">
          {file.map((p) => (
            <li key={`${p.index}-${p.reason}`} className="own-agents__problem" dir="auto">
              {p.reason}
            </li>
          ))}
        </ul>
      )}
      {!own && <p className="own-agents__line">Reading tools.json…</p>}
      {own && tools.length === 0 && !editing && <p className="own-agents__line">No tools of your own yet.</p>}
      {own && tools.length > 0 && (
        <ul className="own-agents__list">
          {tools.map((t, i) => (
            <li key={`${i}-${t.id}`} className="own-agents__item">
              <div className="own-agents__summary">
                <span className="own-agents__name" dir="auto">
                  {t.name}
                </span>
                <span className="own-agents__command" dir="auto">
                  {t.commands.length > 0 ? t.commands.join(", ") : "–"}
                </span>
                {problems
                  .filter((p) => p.index === i)
                  .map((p) => (
                    <span key={p.reason} className="own-agents__problem" dir="auto">
                      {p.reason}
                    </span>
                  ))}
              </div>
              {removing === i ? (
                <div className="own-agents__actions" role="group" aria-label={`Remove ${t.name}?`} onKeyDown={(e) => escapeCancels(e, on.onCancelRemove)}>
                  <span className="own-agents__line own-agents__line--strong" dir="auto">
                    {`Remove ${t.name}?`}
                  </span>
                  <button type="button" className="own-agents__btn" data-own-focus="confirm" autoFocus onClick={on.onConfirmRemove}>
                    Remove
                  </button>
                  <button type="button" className="own-agents__btn" onClick={on.onCancelRemove}>
                    Cancel
                  </button>
                </div>
              ) : (
                <div className="own-agents__actions">
                  <button type="button" className="own-agents__btn" data-own-focus={`edit-${i}`} aria-label={`Edit ${t.name}`} onClick={() => on.onEdit(i)}>
                    Edit
                  </button>
                  <button type="button" className="own-agents__btn" aria-label={`Remove ${t.name}`} onClick={() => on.onRemove(i)}>
                    Remove
                  </button>
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
      {conflict && (
        <div className="own-agents__conflict">
          <p className="own-agents__line own-agents__line--strong">{CONFLICT_TEXT}</p>
          <button type="button" className="own-agents__btn" onClick={on.onReload}>
            Reload
          </button>
        </div>
      )}
      {message && (
        <p className="own-agents__problem" dir="auto">
          {message}
        </p>
      )}
      {editing ? (
        <OwnToolForm editing={editing} onChange={on.onChange} onSave={on.onSave} onCancel={on.onCancel} />
      ) : (
        own && (
          <div className="own-agents__actions">
            <button type="button" className="own-agents__btn" data-own-focus="add" onClick={on.onAdd}>
              Add your own tool
            </button>
          </div>
        )
      )}
    </section>
  );
}

const KINDS: [PromptKind, string][] = [
  ["text", "Text"],
  ["browser", "Browser"],
];

/** The form, in the order it is read and tabbed: name, commands, launch presets, needs-you patterns, port lines, then Save and Cancel. */
function OwnToolForm({ editing, onChange, onSave, onCancel }: { editing: Editing; onChange: (d: ToolDraft) => void; onSave: () => void; onCancel: () => void }) {
  const { draft, problem, busy } = editing;
  useEffect(() => {
    document.getElementById("own-tool-name")?.focus();
  }, []);
  const id = slug(draft.name);
  const invalid = problem ? { "aria-invalid": true as const, "aria-describedby": ERROR_ID } : {};
  const put = <T,>(list: T[], i: number, v: T) => list.map((x, j) => (j === i ? v : x));
  const drop = <T,>(list: T[], i: number) => list.filter((_, j) => j !== i);
  const input = (key: string, label: string, value: string, onInput: (v: string) => void, opts: { hint?: string; invalid?: boolean; placeholder?: string } = {}) => (
    <div className="own-agents__field">
      <label htmlFor={`own-tool-${key}`} className="own-agents__label">
        {label}
      </label>
      <input
        id={`own-tool-${key}`}
        type="text"
        className="own-agents__input"
        value={value}
        placeholder={opts.placeholder}
        autoComplete="off"
        spellCheck={false}
        {...(opts.invalid ? invalid : {})}
        {...(opts.hint ? { "aria-describedby": opts.invalid && problem ? `${ERROR_ID} own-tool-${key}-hint` : `own-tool-${key}-hint` } : {})}
        onChange={(e) => onInput(e.target.value)}
      />
      {opts.hint && (
        <p id={`own-tool-${key}-hint`} className="own-agents__hint">
          {opts.hint}
        </p>
      )}
    </div>
  );
  return (
    <form
      className="own-agents__form"
      aria-label={editing.index === null ? "Add your own tool" : `Edit ${draft.name || "tool"}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (!busy) onSave();
      }}
    >
      {input("name", "Name", draft.name, (v) => onChange({ ...draft, name: v }), { invalid: true, hint: id ? `Id: ${id}` : "The id is made from the name." })}
      {input("commands", "Commands", draft.commands, (v) => onChange({ ...draft, commands: v }), { invalid: true, hint: "Program names, separated by commas, lowercase and without .exe.", placeholder: "mytool, mt" })}

      <fieldset className="own-agents__group">
        <legend className="own-agents__label">Launch presets</legend>
        {draft.presets.map((p, i) => (
          <div key={i} className="own-tools__row">
            {input(`preset-${i}-name`, "Preset name", p.name, (v) => onChange({ ...draft, presets: put(draft.presets, i, { ...p, name: v }) }))}
            {input(`preset-${i}-program`, "Program", p.program, (v) => onChange({ ...draft, presets: put(draft.presets, i, { ...p, program: v }) }))}
            <div className="own-agents__field">
              <label htmlFor={`own-tool-preset-${i}-args`} className="own-agents__label">
                Arguments, one per line
              </label>
              <textarea
                id={`own-tool-preset-${i}-args`}
                className="own-agents__input own-tools__args"
                rows={3}
                value={p.args}
                spellCheck={false}
                aria-describedby={`own-tool-preset-${i}-args-hint`}
                onChange={(e) => onChange({ ...draft, presets: put(draft.presets, i, { ...p, args: e.target.value }) })}
              />
              <p id={`own-tool-preset-${i}-args-hint`} className="own-agents__hint">
                A line like {"{target}"} becomes a field the launcher asks for.
              </p>
            </div>
            {placeholders(p.args).map((f) =>
              input(`preset-${i}-label-${f}`, `Label for ${f}`, p.labels[f] ?? "", (v) => onChange({ ...draft, presets: put(draft.presets, i, { ...p, labels: { ...p.labels, [f]: v } }) }), { placeholder: f }),
            )}
            <div className="own-agents__actions">
              <button type="button" className="own-agents__btn" aria-label={`Remove preset ${p.name || i + 1}`} onClick={() => onChange({ ...draft, presets: drop(draft.presets, i) })}>
                Remove preset
              </button>
            </div>
          </div>
        ))}
        <div className="own-agents__actions">
          <button type="button" className="own-agents__btn" onClick={() => onChange({ ...draft, presets: [...draft.presets, blankPreset()] })}>
            Add a preset
          </button>
        </div>
      </fieldset>

      <fieldset className="own-agents__group">
        <legend className="own-agents__label">Needs-you patterns</legend>
        {draft.needsYou.map((n, i) => (
          <div key={i} className="own-tools__row">
            {input(`need-${i}-pattern`, "Pattern", n.pattern, (v) => onChange({ ...draft, needsYou: put(draft.needsYou, i, { ...n, pattern: v }) }), { hint: "A regular expression run on one line of output." })}
            {input(`need-${i}-phrase`, "Phrase", n.phrase, (v) => onChange({ ...draft, needsYou: put(draft.needsYou, i, { ...n, phrase: v }) }), { hint: "What the session says it is waiting for." })}
            <div className="own-agents__field">
              <label htmlFor={`own-tool-need-${i}-kind`} className="own-agents__label">
                Kind
              </label>
              <select id={`own-tool-need-${i}-kind`} className="own-agents__input" value={n.kind} onChange={(e) => onChange({ ...draft, needsYou: put(draft.needsYou, i, { ...n, kind: e.target.value as PromptKind }) })}>
                {KINDS.map(([value, text]) => (
                  <option key={value} value={value}>
                    {text}
                  </option>
                ))}
              </select>
            </div>
            <div className="own-agents__actions">
              <button type="button" className="own-agents__btn" aria-label={`Remove needs-you pattern ${i + 1}`} onClick={() => onChange({ ...draft, needsYou: drop(draft.needsYou, i) })}>
                Remove pattern
              </button>
            </div>
          </div>
        ))}
        <div className="own-agents__actions">
          <button type="button" className="own-agents__btn" onClick={() => onChange({ ...draft, needsYou: [...draft.needsYou, blankPattern()] })}>
            Add a pattern
          </button>
        </div>
      </fieldset>

      <fieldset className="own-agents__group">
        <legend className="own-agents__label">Port lines</legend>
        {draft.ports.map((p, i) => (
          <div key={i} className="own-tools__row">
            {input(`port-${i}`, "Pattern", p, (v) => onChange({ ...draft, ports: put(draft.ports, i, v) }), { hint: "Capture group 1 is a URL, host:port or a port." })}
            <div className="own-agents__actions">
              <button type="button" className="own-agents__btn" aria-label={`Remove port line ${i + 1}`} onClick={() => onChange({ ...draft, ports: drop(draft.ports, i) })}>
                Remove port line
              </button>
            </div>
          </div>
        ))}
        <div className="own-agents__actions">
          <button type="button" className="own-agents__btn" onClick={() => onChange({ ...draft, ports: [...draft.ports, ""] })}>
            Add a port line
          </button>
        </div>
      </fieldset>

      {problem && (
        <p id={ERROR_ID} className="own-agents__problem" dir="auto">
          {problem}
        </p>
      )}
      <div className="own-agents__actions">
        <button type="submit" className="own-agents__btn" disabled={busy}>
          Save
        </button>
        <button type="button" className="own-agents__btn" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

/** The editor on the core's data. Reads `tools.json` when it mounts; a save names the version it read, and a changed file asks for a reload. */
export function OwnToolsSection({ endpoint, announce }: { endpoint: Endpoint; announce?: (text: string) => void }) {
  const [own, setOwn] = useState<OwnTools | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [removing, setRemoving] = useState<number | null>(null);
  const [conflict, setConflict] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [refused, setRefused] = useState<OwnToolProblem[] | null>(null);
  const [focusNext, setFocusNext] = useState<FormFocus | null>(null);
  const sectionRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!focusNext) return;
    const key = focusNext.to === "add" ? "add" : `edit-${focusNext.index}`;
    sectionRef.current?.querySelector<HTMLElement>(`[data-own-focus="${key}"]`)?.focus();
    setFocusNext(null);
  }, [focusNext, own, editing]);

  const load = useCallback(async () => {
    try {
      setOwn(await ownTools.get(endpoint));
      setMessage(null);
      setRefused(null);
    } catch {
      setMessage("Couldn't read tools.json from Marshell's core. Close this and try again.");
    }
  }, [endpoint]);
  useEffect(() => {
    void load();
  }, [load]);

  const send = async (change: Change, kind: ChangeKind, done: string, index: number | null) => {
    if (!own) return;
    setMessage(null);
    try {
      const saved = await ownTools.save(endpoint, buildSave(own.tools, own.version, change));
      setOwn(saved);
      setEditing(null);
      setRemoving(null);
      setRefused(null);
      setConflict(false);
      if ("remove" in change) setFocusNext(focusAfterRemove(change.remove, saved.tools.length));
      else {
        const id = "add" in change ? change.add.id : change.with.id;
        const at = index ?? saved.tools.findIndex((t) => t.id === id);
        setFocusNext(at >= 0 && at < saved.tools.length ? { to: "edit", index: at } : { to: "add" });
      }
      announce?.(done);
    } catch (e) {
      const out = outcomeOf(e);
      if (out.kind === "conflict") {
        setConflict(true);
        setRemoving(null);
        setEditing((cur) => (cur ? { ...cur, busy: false } : cur));
        announce?.(out.text);
      } else if (out.kind === "refused") {
        const { entries, form } = refusalOf(out.problems, kind, own.tools.length);
        setRefused(entries);
        setRemoving(null);
        const own_ = "replace" in kind ? entries.find((p) => p.index === kind.replace)?.reason : form;
        const generic = "Marshell's core refused this entry.";
        if (editing) setEditing({ ...editing, busy: false, problem: own_ ?? (entries.length === 0 ? generic : "Fix the entries marked in the list first.") });
        else if (entries.length === 0) setMessage(generic);
        announce?.(own_ ?? entries[0]?.reason ?? generic);
      } else {
        if (editing) setEditing({ ...editing, busy: false, problem: out.text });
        else setMessage(out.text);
        setRemoving(null);
        announce?.(out.text);
      }
    }
  };

  const save = () => {
    if (!editing || !own) return;
    setEditing({ ...editing, busy: true, problem: undefined });
    const existing = editing.index === null ? undefined : own.tools[editing.index];
    const profile = profileOf(editing.draft, existing);
    const name = profile.name || "the tool";
    if (editing.index === null) void send({ add: profile }, { add: true }, savedText(name), null);
    else void send({ replace: editing.index, with: profile }, { replace: editing.index }, savedText(name), editing.index);
  };

  return (
    <OwnToolsView
      own={own}
      problems={own ? [...own.problems, ...(refused ?? [])] : undefined}
      editing={editing}
      removing={removing}
      conflict={conflict}
      message={message}
      sectionRef={sectionRef}
      onAdd={() => setEditing({ index: null, draft: blankDraft(), busy: false })}
      onEdit={(i) => {
        const t = own?.tools[i];
        if (t) setEditing({ index: i, draft: draftOf(t), busy: false });
      }}
      onRemove={(i) => setRemoving(i)}
      onConfirmRemove={() => {
        const t = removing === null ? undefined : own?.tools[removing];
        if (t && removing !== null) void send({ remove: removing }, { remove: removing }, removedText(t.name), removing);
      }}
      onCancelRemove={() => {
        if (removing !== null) setFocusNext({ to: "edit", index: removing });
        setRemoving(null);
      }}
      onChange={(draft) => setEditing((cur) => (cur ? { ...cur, draft } : cur))}
      onSave={save}
      onCancel={() => {
        setFocusNext(focusAfterCancel(editing?.index ?? null));
        setEditing(null);
      }}
      onReload={() => {
        setConflict(false);
        // An edit of a listed entry points into a list that may have changed; a new tool can stay.
        setEditing((cur) => (cur && cur.index === null ? cur : null));
        void load();
      }}
    />
  );
}
