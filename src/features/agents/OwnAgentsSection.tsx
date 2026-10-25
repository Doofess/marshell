import { useCallback, useEffect, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import type { OwnAgents } from "../../generated/OwnAgents";
import { ownAgents } from "../../lib/api/catalog";
import { blankForm, buildManifest, buildSave, CONFLICT_TEXT, deriveId, fileProblems, formFromManifest, OWN_MODE_LABELS, OWN_MODES, outcomeOf, type Change, type OwnForm } from "./ownAgents";
import "./OwnAgentsSection.css";

/** The open form: a new agent (`index` null) or one being edited, with the core's reason when it refused the entry. */
export type Editing = { index: number | null; form: OwnForm; busy: boolean; problem?: string };

export const ERROR_ID = "own-agent-error";

export type OwnAgentsViewProps = {
  /** null until the file has been read. */
  own: OwnAgents | null;
  editing: Editing | null;
  /** The file changed since it was read. */
  conflict?: boolean;
  /** A failure that belongs to no field. */
  message?: string | null;
  onAdd: () => void;
  onEdit: (index: number) => void;
  onRemove: (index: number) => void;
  onChange: (form: OwnForm) => void;
  onSave: () => void;
  onCancel: () => void;
  onReload: () => void;
};

/** The "Your own agents" section of the Agents dialog: the list, and the form to add or edit one. Presentational. */
export function OwnAgentsView({ own, editing, conflict, message, ...on }: OwnAgentsViewProps) {
  const problems = own ? own.problems : [];
  const file = own ? fileProblems(own.agents, problems) : [];
  return (
    <section className="own-agents" aria-label="Your own agents">
      <h3 className="own-agents__title">Your own agents</h3>
      {file.length > 0 && (
        <ul className="own-agents__problems">
          {file.map((p) => (
            <li key={`${p.index}-${p.reason}`} className="own-agents__problem" dir="auto">
              {p.reason}
            </li>
          ))}
        </ul>
      )}
      {!own && <p className="own-agents__line">Reading agents.json…</p>}
      {own && own.agents.length === 0 && !editing && <p className="own-agents__line">No agents of your own yet.</p>}
      {own && own.agents.length > 0 && (
        <ul className="own-agents__list">
          {own.agents.map((a, i) => (
            <li key={`${i}-${a.id}`} className="own-agents__item">
              <div className="own-agents__summary">
                <span className="own-agents__name" dir="auto">
                  {a.name}
                </span>
                <span className="own-agents__command" dir="auto">
                  {a.commands[0] ?? "–"}
                </span>
                {problems
                  .filter((p) => p.index === i)
                  .map((p) => (
                    <span key={p.reason} className="own-agents__problem" dir="auto">
                      {p.reason}
                    </span>
                  ))}
              </div>
              <div className="own-agents__actions">
                <button type="button" className="own-agents__btn" aria-label={`Edit ${a.name}`} onClick={() => on.onEdit(i)}>
                  Edit
                </button>
                <button type="button" className="own-agents__btn" aria-label={`Remove ${a.name}`} onClick={() => on.onRemove(i)}>
                  Remove
                </button>
              </div>
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
      {message && <p className="own-agents__problem">{message}</p>}
      {editing ? (
        <OwnAgentForm editing={editing} onChange={on.onChange} onSave={on.onSave} onCancel={on.onCancel} />
      ) : (
        own && (
          <div className="own-agents__actions">
            <button type="button" className="own-agents__btn" onClick={on.onAdd}>
              Add an agent
            </button>
          </div>
        )
      )}
    </section>
  );
}

/** The form. Its fields are in the order they are read and tabbed: name, command, first prompt, continue, the modes, program names, then Save and Cancel. */
function OwnAgentForm({ editing, onChange, onSave, onCancel }: { editing: Editing; onChange: (form: OwnForm) => void; onSave: () => void; onCancel: () => void }) {
  const { form, problem, busy } = editing;
  const set = <K extends keyof OwnForm>(key: K, value: OwnForm[K]) => onChange({ ...form, [key]: value });
  const setMode = (mode: (typeof OWN_MODES)[number], value: string) => onChange({ ...form, modes: { ...form.modes, [mode]: value }, manualNoFlags: mode === "manual" && value.trim() ? false : form.manualNoFlags });
  const id = deriveId(form.name);
  // A refusal is about the entry, not one field: it is tied to the two fields the person is most likely to fix.
  const invalid = problem ? { "aria-invalid": true as const, "aria-describedby": ERROR_ID } : {};
  const field = (key: string, label: string, value: string, onInput: (v: string) => void, opts: { hint?: string; invalid?: boolean; disabled?: boolean; placeholder?: string } = {}) => (
    <div className="own-agents__field">
      <label htmlFor={`own-agent-${key}`} className="own-agents__label">
        {label}
      </label>
      <input
        id={`own-agent-${key}`}
        type="text"
        className="own-agents__input"
        value={value}
        placeholder={opts.placeholder}
        disabled={opts.disabled}
        autoComplete="off"
        spellCheck={false}
        {...(opts.invalid ? invalid : {})}
        {...(opts.hint ? { "aria-describedby": opts.invalid && problem ? `${ERROR_ID} own-agent-${key}-hint` : `own-agent-${key}-hint` } : {})}
        onChange={(e) => onInput(e.target.value)}
      />
      {opts.hint && (
        <p id={`own-agent-${key}-hint`} className="own-agents__hint">
          {opts.hint}
        </p>
      )}
    </div>
  );
  return (
    <form
      className="own-agents__form"
      aria-label={editing.index === null ? "Add an agent" : `Edit ${form.name || "agent"}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (!busy) onSave();
      }}
    >
      {field("name", "Name", form.name, (v) => set("name", v), { invalid: true, hint: id ? `Id: ${id}` : "The id is made from the name." })}
      {field("command", "Command", form.command, (v) => set("command", v), { invalid: true, hint: "A program name on PATH, or a full path.", placeholder: "my-agent" })}
      <fieldset className="own-agents__group">
        <legend className="own-agents__label">First prompt</legend>
        <div className="own-agents__choices">
          {(
            [
              ["none", "Not at all"],
              ["positional", "As the last word"],
              ["flag", "After a flag"],
            ] as const
          ).map(([value, text]) => (
            <label key={value} className="own-agents__choice">
              <input type="radio" name="own-agent-prompt" value={value} checked={form.prompt === value} onChange={() => set("prompt", value)} />
              <span>{text}</span>
            </label>
          ))}
        </div>
        {form.prompt === "flag" && field("prompt-flag", "Prompt flag", form.promptFlag, (v) => set("promptFlag", v), { placeholder: "-p" })}
      </fieldset>
      {field("continue", "Continue flags", form.continueFlags, (v) => set("continueFlags", v), { hint: "Blank if the agent can't continue the last conversation. Put a flag with a space in quotes." })}
      {OWN_MODES.map((mode) => (
        <div key={mode} className="own-agents__modefield">
          {field(`mode-${mode}`, `${OWN_MODE_LABELS[mode]} flags`, form.modes[mode], (v) => setMode(mode, v), {
            hint: mode === "manual" ? undefined : "Blank means this mode isn't offered.",
            disabled: mode === "manual" && form.manualNoFlags,
          })}
          {mode === "manual" && (
            <label className="own-agents__choice">
              <input type="checkbox" checked={form.manualNoFlags} onChange={(e) => onChange({ ...form, manualNoFlags: e.target.checked, modes: e.target.checked ? { ...form.modes, manual: "" } : form.modes })} />
              <span>Manual needs no flags: the agent asks before it runs tools</span>
            </label>
          )}
        </div>
      ))}
      {field("exe", "Program names to recognise", form.exeNames, (v) => set("exeNames", v), { hint: "Separate with commas or spaces. Marshell matches a running session to the agent by these." })}
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

/** The section on the core's data. Reads `agents.json` when it mounts; a save names the version it read, and a changed file asks for a reload. */
export function OwnAgentsSection({ endpoint, onNotice }: { endpoint: Endpoint; onNotice?: (text: string) => void }) {
  const [own, setOwn] = useState<OwnAgents | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [conflict, setConflict] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setOwn(await ownAgents.get(endpoint));
      setMessage(null);
    } catch {
      setMessage("Couldn't read agents.json from Marshell's core. Close this and try again.");
    }
  }, [endpoint]);
  useEffect(() => {
    void load();
  }, [load]);

  const send = async (change: Change, done: string, index: number | null) => {
    if (!own) return;
    setMessage(null);
    try {
      const saved = await ownAgents.save(endpoint, buildSave(own.agents, own.version, change));
      setOwn(saved);
      setEditing(null);
      setConflict(false);
      onNotice?.(done);
    } catch (e) {
      const out = outcomeOf(e);
      if (out.kind === "conflict") {
        setConflict(true);
        setEditing((cur) => (cur ? { ...cur, busy: false } : cur));
        onNotice?.(out.text);
      } else if (out.kind === "refused") {
        const sent = "remove" in change ? own.agents.length - 1 : index === null ? own.agents.length : index;
        const mine = out.problems.find((p) => p.index === sent) ?? out.problems[0];
        const reason = mine?.reason ?? "Marshell's core refused this entry.";
        if (editing) setEditing({ ...editing, busy: false, problem: reason });
        else setMessage(reason);
        onNotice?.(reason);
      } else {
        if (editing) setEditing({ ...editing, busy: false, problem: out.text });
        else setMessage(out.text);
        onNotice?.(out.text);
      }
    }
  };

  const save = () => {
    if (!editing || !own) return;
    setEditing({ ...editing, busy: true, problem: undefined });
    const manifest = buildManifest(editing.form);
    const change: Change = editing.index === null ? { add: manifest } : { replace: editing.index, with: manifest };
    void send(change, `${manifest.name || "The agent"} saved.`, editing.index);
  };

  return (
    <OwnAgentsView
      own={own}
      editing={editing}
      conflict={conflict}
      message={message}
      onAdd={() => setEditing({ index: null, form: blankForm(), busy: false })}
      onEdit={(i) => {
        const a = own?.agents[i];
        if (a) setEditing({ index: i, form: formFromManifest(a), busy: false });
      }}
      onRemove={(i) => {
        const a = own?.agents[i];
        if (a) void send({ remove: i }, `${a.name} removed.`, i);
      }}
      onChange={(form) => setEditing((cur) => (cur ? { ...cur, form } : cur))}
      onSave={save}
      onCancel={() => setEditing(null)}
      onReload={() => {
        setConflict(false);
        // An edit of a listed entry points into a list that may have changed; a new agent can stay.
        setEditing((cur) => (cur && cur.index === null ? cur : null));
        void load();
      }}
    />
  );
}
