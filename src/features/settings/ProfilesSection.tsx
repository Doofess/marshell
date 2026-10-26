import { useEffect, useEffectEvent, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import type { Endpoint } from "../../generated/Endpoint";
import type { Profile } from "../../generated/Profile";
import { profiles as profilesApi } from "../../lib/api/profiles";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { useAppearance } from "../appearance/AppearanceProvider";
import { changeCount, commandRoundTrips, duplicateEnvKeys, forSave, joinCommand, profileKind, splitCommand, type ProfilePatch } from "./profilesDoc";
import { Switch } from "../../components/Switch/Switch";
import { isDirty, ProfilesStore, type ProfilesState, type ScriptView } from "./ProfilesStore";
import { ChangeBar } from "./ChangeBar";
import { FieldRow } from "./FieldRow";
import type { SettingField } from "./fields";
import "./Settings.css";
import "./ProfilesSection.css";

/** The section may be left when it holds no unsaved edits (also while it is still loading or failed to load). */
export const canClose = (state: ProfilesState): boolean => !isDirty(state);

export type ProfilesHandlers = {
  onSelect: (id: string) => void;
  onPatch: (patch: ProfilePatch) => void;
  onAdd: () => void;
  onRemove: () => void;
  onMakeDefault: () => void;
  onReset: () => void;
  onSave: () => void;
  onDiscard: () => void;
  onToggleScript: () => void;
};

const field = (key: string, label: string, help: string): SettingField => ({ category: "profiles", key, label, help, kind: "text", scope: "marshell", applies: "new-sessions" });
const NAME = field("name", "Name", "How the profile is listed when you start a session.");
const COMMAND = field("command", "Command", "The program and its arguments. Put quotes around a path that has spaces.");
const FOLDER = field("cwd", "Folder", "Where sessions start. Empty means your home folder.");

/**
 * The command is typed as one line; the profile keeps the argv. What the person typed is shown while it still reads as the
 * argv (so a trailing space or an open quote is not taken away mid-typing); once the argv changes from outside (Reset,
 * Discard, another profile) the line is written afresh from it. Nothing is set while rendering.
 * An argv the line cannot write back (an argument with a double quote, as a hand-edited profiles.json may hold) is shown
 * read-only with a note, so it is never rewritten without the person's say: Reset or editing the file changes it.
 */
function CommandRow({ command, onPatch }: { command: string[]; onPatch: (patch: ProfilePatch) => void }) {
  const [typed, setTyped] = useState<string | null>(null);
  if (!commandRoundTrips(command)) {
    return (
      <>
        <FieldRow field={COMMAND} value={command.join(" ")} onChange={() => {}} readOnly />
        <li className="field__why profiles__note">An argument of this command has a quote mark, which this box can&apos;t write, so it is shown as it is and can&apos;t be edited here. Edit profiles.json, or Reset a detected profile.</li>
      </>
    );
  }
  const shown = typed !== null && JSON.stringify(splitCommand(typed)) === JSON.stringify(command) ? typed : joinCommand(command);
  return (
    <FieldRow
      field={COMMAND}
      value={shown}
      onChange={(t) => {
        setTyped(t);
        onPatch({ command: splitCommand(t) });
      }}
    />
  );
}

function EnvRows({ env, onPatch }: { env: Array<[string, string]>; onPatch: (patch: ProfilePatch) => void }) {
  const set = (i: number, pair: [string, string]) => onPatch({ env: env.map((p, j) => (j === i ? pair : p)) });
  return (
    <li className="field">
      <div className="field__head">
        <span className="field__label" id="profile-env-label">
          Environment variables
        </span>
      </div>
      <p className="field__help">Set in every session started from this profile, on top of Marshell&apos;s own.</p>
      <ul className="profiles__env" aria-labelledby="profile-env-label">
        {env.map(([k, v], i) => (
          <li key={i} className="profiles__env-row">
            <input className="field__control profiles__env-input" type="text" aria-label="Variable name" value={k} spellCheck={false} onChange={(e) => set(i, [e.target.value, v])} />
            <input className="field__control profiles__env-input" type="text" aria-label="Value" value={v} spellCheck={false} onChange={(e) => set(i, [k, e.target.value])} />
            <button type="button" className="settings__btn" aria-label={`Remove variable ${k || i + 1}`} onClick={() => onPatch({ env: env.filter((_, j) => j !== i) })}>
              Remove
            </button>
          </li>
        ))}
      </ul>
      <div>
        <button type="button" className="settings__btn" onClick={() => onPatch({ env: [...env, ["", ""]] })}>
          Add variable
        </button>
      </div>
      {duplicateEnvKeys(env).map((k) => (
        <p key={k} className="field__why">
          {k} is listed twice. Remove one.
        </p>
      ))}
      <p className="field__applies">Applies to new sessions.</p>
    </li>
  );
}

/** The integration switch, and a read-only look at what it loads: nothing here writes a file or the person's shell profile. */
function IntegrationRow({ on, script, onPatch, onToggleScript }: { on: boolean; script: ScriptView; onPatch: (patch: ProfilePatch) => void; onToggleScript: () => void }) {
  const open = script.phase !== "closed";
  return (
    <li className="field">
      <Switch label="Shell integration" checked={on} onChange={(checked) => onPatch({ shell_integration: checked })} />
      <p className="field__help">Marshell loads a small script of its own when the shell starts, so it can tell when a command runs, how it ended, and where each prompt is. Your shell&apos;s own files are left as they are.</p>
      <div>
        <button type="button" className="settings__btn" aria-expanded={open} aria-controls="profile-script" onClick={onToggleScript}>
          View script
        </button>
      </div>
      {open && (
        <div id="profile-script" className="profiles__script">
          {script.phase === "loading" && <p className="profiles__status">Loading the script…</p>}
          {script.phase === "failed" && <p className="field__why">Couldn&apos;t load the script.</p>}
          {script.phase === "ready" && !script.info.applies && <p className="field__why">{script.info.reason}</p>}
          {script.phase === "ready" && script.info.applies && (
            <>
              {script.info.script_path && (
                <p className="field__help">
                  <code className="profiles__path" dir="ltr">
                    {script.info.script_path}
                  </code>
                </p>
              )}
              <pre className="profiles__script-text" tabIndex={0} aria-label="Integration script" dir="ltr">{script.info.script}</pre>
            </>
          )}
        </div>
      )}
      <p className="field__applies">Applies to new sessions.</p>
    </li>
  );
}

/** The profile's own terminal theme; "" is no override, so its terminals follow the Appearance setting. */
function TerminalThemeRow({ value, themes, onPatch }: { value: string | undefined; themes: { id: string; name: string }[]; onPatch: (patch: ProfilePatch) => void }) {
  return (
    <li className="field">
      <label className="field__label" htmlFor="profile-terminal-theme">
        Terminal theme
      </label>
      <p id="profile-terminal-theme-help" className="field__help">
        Overrides the Appearance setting for this profile&apos;s terminals.
      </p>
      <select id="profile-terminal-theme" className="field__control" aria-describedby="profile-terminal-theme-help" value={value ?? ""} onChange={(e) => onPatch({ terminal_theme: e.target.value === "" ? undefined : e.target.value })}>
        <option value="">Same as the app setting</option>
        <option value="dark">Dark</option>
        <option value="light">Light</option>
        {themes.map((t) => (
          <option key={t.id} value={t.id}>
            {t.name}
          </option>
        ))}
      </select>
      <p className="field__applies">Applies to new sessions.</p>
    </li>
  );
}

const KIND_NOTE = {
  custom: "A profile you added.",
  detected: "Found on this machine. It can't be deleted: detection would bring it back.",
  edited: "Found on this machine, with your changes. Reset puts back what was found.",
} as const;

/**
 * The shell profiles: the list on one side, the selected profile's form on the other. Presentational: ProfilesSection owns the
 * store. Nothing is saved until Save; the bar shows what the core said when it refused.
 */
export function ProfilesView({ state, handlers, themes = [] }: { state: ProfilesState; handlers: ProfilesHandlers; themes?: { id: string; name: string }[] }) {
  const { draft, detected, selectedId } = state;
  const selected: Profile | undefined = draft?.profiles.find((p) => p.id === selectedId);
  const kind = selected ? profileKind(selected, detected) : "custom";
  const dirty = isDirty(state);
  const count = state.saved && draft ? changeCount(state.saved, forSave(draft)) : 0;
  let body;
  if (state.phase === "loading") {
    body = <p className="profiles__status">Loading the profiles…</p>;
  } else if (state.phase === "failed" || !draft || !selected) {
    body = <EmptyState title="The profiles didn't load" body={`Couldn't load the profiles: ${state.loadError ?? "no profile found"}`} />;
  } else {
    body = (
      <div className="settings__layout">
        <nav aria-label="Profiles" className="settings__nav profiles__list">
          <ul>
            {draft.profiles.map((p) => {
              const k = profileKind(p, detected);
              return (
                <li key={p.id}>
                  <button type="button" {...selectedProps(p.id === selectedId, "current", "settings__nav-item profiles__item")} onClick={() => handlers.onSelect(p.id)}>
                    <span className="profiles__name" dir="auto">
                      {p.name}
                    </span>
                    {p.id === draft.default_id && <span className="profiles__badge">Default</span>}
                    {k !== "custom" && <span className="profiles__badge">Detected</span>}
                    {k === "edited" && <span className="profiles__badge">Edited</span>}
                  </button>
                </li>
              );
            })}
          </ul>
          <button type="button" className="settings__btn" onClick={handlers.onAdd}>
            Add profile
          </button>
          <p className="field__help">Copies the selected profile.</p>
        </nav>
        <div className="settings__body">
          <section className="profiles__form" aria-label={`Edit ${selected.name}`}>
            <ul className="settings__fields">
              <FieldRow field={NAME} value={selected.name} onChange={(name) => handlers.onPatch({ name })} />
              <CommandRow command={selected.command} onPatch={handlers.onPatch} />
              <FieldRow field={FOLDER} value={selected.cwd ?? ""} onChange={(cwd) => handlers.onPatch({ cwd: cwd === "" ? null : cwd })} />
              <EnvRows env={selected.env} onPatch={handlers.onPatch} />
              <IntegrationRow on={selected.shell_integration} script={state.script} onPatch={handlers.onPatch} onToggleScript={handlers.onToggleScript} />
              <TerminalThemeRow value={selected.terminal_theme} themes={themes} onPatch={handlers.onPatch} />
            </ul>
            <div className="profiles__actions">
              <button type="button" className="settings__btn" disabled={selected.id === draft.default_id} onClick={handlers.onMakeDefault}>
                Make default
              </button>
              {kind !== "custom" && (
                <button type="button" className="settings__btn" disabled={kind === "detected"} onClick={handlers.onReset}>
                  Reset
                </button>
              )}
              <button type="button" className="settings__btn" disabled={kind !== "custom" || draft.profiles.length < 2} onClick={handlers.onRemove}>
                Delete
              </button>
            </div>
            <p className="field__help">{KIND_NOTE[kind]}</p>
          </section>
          <ChangeBar
            path="~/.marshell/profiles.json"
            opened=""
            count={count}
            state={{ changed: dirty }}
            error={state.error}
            save={{ busy: state.saving, onSave: handlers.onSave, onDiscard: handlers.onDiscard }}
          />
        </div>
      </div>
    );
  }
  return (
    <div className="settings profiles">
      {body}
      {state.justSaved && <p className="profiles__saved">Saved.</p>}
    </div>
  );
}

/** The shell profiles on the core. Mounted when the section is first shown, so it loads then. */
export function ProfilesSection({ endpoint, announce, onSaved, onCanClose }: { endpoint: Endpoint; announce?: (text: string) => void; onSaved?: () => void; onCanClose?: (can: boolean) => void }) {
  const store = useMemo(() => new ProfilesStore(profilesApi, endpoint, onSaved), [endpoint, onSaved]);
  useEffect(() => {
    void store.load();
  }, [store]);
  const state = useSyncExternalStore(store.subscribe, store.getState);
  const { appearance } = useAppearance();
  // The window has one live region (its announcer); the result of Save goes there instead of a region of this section.
  const say = useEffectEvent((text: string) => announce?.(text));
  // The owner needs to know too, so it can leave unsaved edits alone as the old Close did.
  const tell = useEffectEvent((can: boolean) => onCanClose?.(can));
  const can = canClose(state);
  useEffect(() => tell(can), [can]);
  useEffect(() => {
    if (state.justSaved) say("Profiles saved.");
  }, [state.justSaved]);
  useEffect(() => {
    if (state.error) say(`Couldn't save the profiles. ${state.error}`);
  }, [state.error]);
  const handlers: ProfilesHandlers = useMemo(
    () => ({
      onSelect: (id) => store.select(id),
      onPatch: (patch) => store.patch(patch),
      onAdd: () => store.add(),
      onRemove: () => store.remove(),
      onMakeDefault: () => store.makeDefault(),
      onReset: () => store.reset(),
      onSave: () => void store.save(),
      onDiscard: () => store.discard(),
      onToggleScript: () => void store.toggleScript(),
    }),
    [store],
  );
  return <ProfilesView state={state} handlers={handlers} themes={appearance.imported_themes} />;
}

/** Temporary until the Settings page is wired in (slice 6e1 Task 9). */
export function ProfilesDialog({ endpoint, onClose, onNotice, onSaved }: { endpoint: Endpoint; onClose: () => void; onNotice?: (text: string) => void; onSaved?: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const [closable, setClosable] = useState(true);
  const closableNow = useRef(true);
  // Escape does nothing while there are unsaved edits (the change bar stays); Close is off then too.
  useDialogFocus(dialog, dialog, () => {
    if (closableNow.current) onClose();
  });
  const onCanClose = (can: boolean) => {
    closableNow.current = can;
    setClosable(can);
  };
  const close = (
    <button type="button" className="settings__btn" disabled={!closable} onClick={onClose}>
      Close
    </button>
  );
  return (
    <div ref={dialog} className="profiles-dialog" role="dialog" aria-modal="true" aria-label="Shell profiles" tabIndex={-1}>
      <Page title="Shell profiles" subtitle="The shells Marshell starts sessions in." actions={close}>
        <ProfilesSection endpoint={endpoint} announce={onNotice} onSaved={onSaved} onCanClose={onCanClose} />
        {!closable && <p className="field__help profiles__saved">Save or discard the changes to close.</p>}
      </Page>
    </div>
  );
}
