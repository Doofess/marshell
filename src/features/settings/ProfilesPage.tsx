import { useEffect, useEffectEvent, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import type { Endpoint } from "../../generated/Endpoint";
import type { Profile } from "../../generated/Profile";
import { profiles as profilesApi } from "../../lib/api/profiles";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { changeCount, forSave, joinCommand, profileKind, splitCommand, type ProfilePatch } from "./profilesDoc";
import { isDirty, ProfilesStore, type ProfilesState } from "./ProfilesStore";
import { ChangeBar } from "./ChangeBar";
import { FieldRow } from "./FieldRow";
import type { SettingField } from "./fields";
import "./Settings.css";
import "./ProfilesPage.css";

export type ProfilesHandlers = {
  onSelect: (id: string) => void;
  onPatch: (patch: ProfilePatch) => void;
  onAdd: () => void;
  onRemove: () => void;
  onMakeDefault: () => void;
  onReset: () => void;
  onSave: () => void;
  onDiscard: () => void;
  onClose?: () => void;
};

const field = (key: string, label: string, help: string): SettingField => ({ category: "profiles", key, label, help, kind: "text", scope: "marshell", applies: "new-sessions" });
const NAME = field("name", "Name", "How the profile is listed when you start a session.");
const COMMAND = field("command", "Command", "The program and its arguments. Put quotes around a path that has spaces.");
const FOLDER = field("cwd", "Folder", "Where sessions start. Empty means your home folder.");

/** The command is typed as one line; the profile keeps the argv. The text is the person's own until the argv changes from outside (Reset, Discard, another profile). */
function CommandRow({ command, onPatch }: { command: string[]; onPatch: (patch: ProfilePatch) => void }) {
  const [text, setText] = useState(() => joinCommand(command));
  if (JSON.stringify(splitCommand(text)) !== JSON.stringify(command)) setText(joinCommand(command));
  return (
    <FieldRow
      field={COMMAND}
      value={text}
      onChange={(t) => {
        setText(t);
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
 * The shell profiles: the list on one side, the selected profile's form on the other. Presentational: ProfilesPage owns the
 * store. Nothing is saved until Save; the bar shows what the core said when it refused.
 */
export function ProfilesPageView({ state, handlers, level = 2 }: { state: ProfilesState; handlers: ProfilesHandlers; level?: 1 | 2 }) {
  const { draft, detected, selectedId } = state;
  const selected: Profile | undefined = draft?.profiles.find((p) => p.id === selectedId);
  const kind = selected ? profileKind(selected, detected) : "custom";
  const dirty = isDirty(state);
  const count = state.saved && draft ? changeCount(state.saved, forSave(draft)) : 0;
  const close = handlers.onClose && (
    <button type="button" className="settings__btn" onClick={handlers.onClose}>
      Close
    </button>
  );

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
      <Page title="Shell profiles" subtitle="The shells Marshell starts sessions in." level={level} actions={close}>
        {body}
        {state.justSaved && <p className="profiles__saved">Saved.</p>}
      </Page>
    </div>
  );
}

/** The shell profiles on the core. Mounted only while open, so each opening loads afresh. */
export function ProfilesPage({ endpoint, onClose, onNotice }: { endpoint: Endpoint; onClose?: () => void; onNotice?: (text: string) => void }) {
  const store = useMemo(() => new ProfilesStore(profilesApi, endpoint), [endpoint]);
  useEffect(() => {
    void store.load();
  }, [store]);
  const state = useSyncExternalStore(store.subscribe, store.getState);
  // The window has one live region (its announcer); the result of Save goes there instead of a region of this page.
  const say = useEffectEvent((text: string) => onNotice?.(text));
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
      onClose,
    }),
    [store, onClose],
  );
  return <ProfilesPageView state={state} handlers={handlers} />;
}

/** The page as a dialog over the window: focus moves in, Tab stays inside, Escape closes and focus goes back to where it was. */
export function ProfilesDialog({ endpoint, onClose, onNotice }: { endpoint: Endpoint; onClose: () => void; onNotice?: (text: string) => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  useDialogFocus(dialog, dialog, onClose);
  return (
    <div ref={dialog} className="profiles-dialog" role="dialog" aria-modal="true" aria-label="Shell profiles" tabIndex={-1}>
      <ProfilesPage endpoint={endpoint} onClose={onClose} onNotice={onNotice} />
    </div>
  );
}
