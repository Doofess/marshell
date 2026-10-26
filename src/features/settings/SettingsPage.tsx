import { useId } from "react";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import { EMPTY_STATES } from "../onboarding/emptyStates";
import { pendingChanges } from "./edit";
import { parseRaw } from "./raw";
import { ChangeBar } from "./ChangeBar";
import { FieldRow } from "./FieldRow";
import { RawEditor } from "./RawEditor";
import { SettingsNav } from "./SettingsNav";
import { fileReason } from "./fields";
import { SETTINGS_FIELDS, SETTINGS_NAV, SETTINGS_TEXT } from "./fixtures";
import { settingsPath, type SettingsState } from "./settingsUi";
import "./Settings.css";

/**
 * The settings page: sections on one side, the section's values on the other, each with where it comes from. A value can be
 * edited as a form or as the raw file. Every change is shown as the exact diff, with the backup and the way to undo,
 * before Apply. A file that does not parse, is read-only, or changed on disk is never overwritten.
 */
export function SettingsPage({ category, mode, state, edits, opened = SETTINGS_TEXT, level = 1 }: { category: string; mode: "form" | "raw"; state: SettingsState; edits?: Record<string, unknown>; opened?: string; level?: 1 | 2 }) {
  const modeId = useId();
  const fields = SETTINGS_FIELDS.filter((f) => f.category === category);
  const path = settingsPath(category);
  const file = { readOnly: state.readOnly, unparsable: state.unparsable };
  const parsed = state.unparsable ? parseRaw(opened) : undefined;
  const reason = fileReason(file, path, parsed && !parsed.ok ? { line: parsed.line, column: parsed.column } : undefined);
  // Only the edits that really differ from what is set count and are previewed.
  const pending = edits ? pendingChanges(fields, edits) : undefined;
  const isAgent = category === "codex" || category === "antigravity";
  const empty = EMPTY_STATES.find((e) => e.id === "no-agents")!;

  return (
    <div className="settings">
      <Page
        title="Settings"
        level={level}
        actions={
          <div className="edit-mode" role="radiogroup" aria-labelledby={modeId}>
            <span id={modeId} className="edit-mode__label">
              Edit as
            </span>
            <label className="edit-mode__option">
              <input type="radio" name={`${modeId}-edit-as`} defaultChecked={mode === "form"} disabled={state.unparsable} />
              <span>Form</span>
            </label>
            <label className="edit-mode__option">
              <input type="radio" name={`${modeId}-edit-as`} defaultChecked={mode === "raw"} />
              <span>Raw</span>
            </label>
          </div>
        }
      >
        <div className="settings__layout">
          <SettingsNav items={SETTINGS_NAV} current={category} />
          <div className="settings__body">
            {reason && <p className="settings__banner" data-tone="blocked">{reason}</p>}
            {state.conflict && (
              <div className="settings__banner" data-tone="conflict">
                <p>The file changed on disk since you opened it.</p>
                <div className="settings__banner-actions">
                  <button type="button" className="settings__btn">
                    Review the difference
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
              <EmptyState title="Nothing to set here yet" body="Snapshots and removal will be listed here once Marshell has changed one of your files." />
            )}
            <ChangeBar path={path} opened={opened} edits={pending} count={pending ? Object.keys(pending).length : undefined} state={state} />
          </div>
        </div>
      </Page>
    </div>
  );
}
