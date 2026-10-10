import { useId } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import { EMPTY_STATES } from "../onboarding/emptyStates";
import { ChangeBar } from "./ChangeBar";
import { FieldRow } from "./FieldRow";
import { RawEditor } from "./RawEditor";
import { fileReason } from "./fields";
import { SETTINGS_FIELDS, SETTINGS_NAV, SETTINGS_TEXT } from "./fixtures";
import { settingsPath, type SettingsState } from "./settingsUi";
import "./Settings.css";

/**
 * The settings page: sections on one side, the section's values on the other, each with where it comes from. A value can be
 * edited as a form or as the raw file. Every change is shown as the exact diff, with the backup and the way to undo,
 * before Apply. A file that does not parse, is read-only, or changed on disk is never overwritten.
 */
export function SettingsPage({ category, mode, state, edits, opened = SETTINGS_TEXT }: { category: string; mode: "form" | "raw"; state: SettingsState; edits?: Record<string, unknown>; opened?: string }) {
  const modeId = useId();
  const fields = SETTINGS_FIELDS.filter((f) => f.category === category);
  const path = settingsPath(category);
  const file = { readOnly: state.readOnly, unparsable: state.unparsable };
  const reason = fileReason(file);
  const isAgent = category === "codex" || category === "gemini";
  const empty = EMPTY_STATES.find((e) => e.id === "no-agents")!;

  return (
    <div className="settings">
      <Page
        title="Settings"
        level={1}
        actions={
          <div className="edit-mode" role="radiogroup" aria-labelledby={modeId}>
            <span id={modeId} className="edit-mode__label">
              Edit as
            </span>
            <label className="edit-mode__option">
              <input type="radio" name="edit-as" defaultChecked={mode === "form"} disabled={state.unparsable} />
              <span>Form</span>
            </label>
            <label className="edit-mode__option">
              <input type="radio" name="edit-as" defaultChecked={mode === "raw"} />
              <span>Raw</span>
            </label>
          </div>
        }
      >
        <div className="settings__layout">
          <nav aria-label="Settings sections" className="settings__nav">
            <ul>
              {SETTINGS_NAV.map((n) => (
                <li key={n.id}>
                  <button type="button" {...selectedProps(n.id === category, "current", "settings__nav-item")}>
                    {n.label}
                  </button>
                </li>
              ))}
            </ul>
          </nav>
          <div className="settings__body">
            {reason && <p className="settings__banner" data-tone="blocked">{reason}</p>}
            {state.conflict && (
              <div className="settings__banner" data-tone="conflict">
                <p>The file changed on disk since you opened it.</p>
                <div className="settings__banner-actions">
                  <button type="button" className="settings__btn">
                    Show differences
                  </button>
                  <button type="button" className="settings__btn">
                    Keep mine
                  </button>
                  <button type="button" className="settings__btn">
                    Reload
                  </button>
                </div>
              </div>
            )}
            {mode === "raw" ? (
              <RawEditor path={path} text={opened} readOnly={Boolean(reason)} />
            ) : fields.length > 0 ? (
              <ul className="settings__fields">
                {fields.map((f) => (
                  <FieldRow key={f.key} field={f} file={file} value={edits?.[f.key]} />
                ))}
              </ul>
            ) : isAgent ? (
              <EmptyState title={empty.title} body={empty.body} action={empty.action ? { label: empty.action } : undefined} />
            ) : (
              <EmptyState title="Nothing to set here yet" body="Snapshots, backups and removal are kept here in a later phase." />
            )}
            <ChangeBar path={path} opened={opened} edits={edits} state={state} />
          </div>
        </div>
      </Page>
    </div>
  );
}
