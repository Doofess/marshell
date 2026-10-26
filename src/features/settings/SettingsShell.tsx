import type { ReactNode } from "react";
import { Page } from "../../components/Page/Page";
import { SettingsNav } from "./SettingsNav";
import { SETTINGS_SECTIONS, type SettingsSection } from "./settingsSections";
import "./Settings.css";
import "./SettingsShell.css";

/** What Esc does on the page: close it, unless something inside (a popup, a field) already handled that Esc. */
export function settingsKey(e: { key: string; defaultPrevented: boolean }): "close" | null {
  return e.key === "Escape" && !e.defaultPrevented ? "close" : null;
}

/**
 * The Settings page: the section nav beside (or, when narrow, above) the shown section. Presentational: the caller holds
 * the section, moves focus to the heading on open and listens for Esc with `settingsKey`. The body scrolls in `.page__body`.
 */
export function SettingsShell({ section, onSection, onClose, children }: { section: SettingsSection; onSection(id: SettingsSection): void; onClose(): void; children: ReactNode }) {
  const label = SETTINGS_SECTIONS.find((s) => s.id === section)?.label ?? "";
  return (
    <div className="settings-shell">
      <Page
        title="Settings"
        level={1}
        actions={
          <button type="button" className="settings__btn" onClick={onClose}>
            Back to sessions
          </button>
        }
      >
        <div className="settings-shell__layout">
          <SettingsNav items={SETTINGS_SECTIONS} current={section} onPick={onSection} className="settings__nav settings-shell__nav" ariaCurrent="page" />
          <div className="settings-shell__section">
            <h2 tabIndex={-1} className="settings-shell__heading">
              {label}
            </h2>
            {children}
          </div>
        </div>
      </Page>
    </div>
  );
}
