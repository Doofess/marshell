import { useEffect, useRef, useState } from "react";
import { Switch } from "../../components/Switch/Switch";
import type { NotifyChannel } from "../../generated/NotifyChannel";
import type { NotifySettings } from "../../generated/NotifySettings";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { SettingsReadError, type SettingsSync } from "../appearance/settingsSync";
import { failureText } from "../agents/useInstallFlow";
import { readNotificationLink, setNotificationLink } from "./notificationLink";
import "./NotificationsSection.css";

export const SETTINGS_UNREADABLE = "Couldn't read your settings. Close this and try again.";

/** The three events that can show a banner, with the name each goes by in the section. */
const EVENTS: { key: "needs_you" | "done" | "error"; label: string }[] = [
  { key: "needs_you", label: "Needs you" },
  { key: "done", label: "Done" },
  { key: "error", label: "Error" },
];

export type NotificationsHandlers = {
  /** The settings with one change made; the owner saves them. */
  onChange: (next: NotifySettings) => void;
  /** The close button's tray choice: true keeps Marshell running in the tray. */
  onCloseToTray?: (keepInTray: boolean) => void;
  /** The notification-centre switch: true registers the marshell:// link. */
  onLink?: (on: boolean) => void;
};

/**
 * Settings → Notifications: which events show a banner or play a sound, whether a finished subagent does, and the quiet hours.
 * It also holds the one setting that is not a notification, what the close button does, since a person who turns off the tray looks here.
 * Presentational: `NotificationsSection` reads and saves, this draws. `settings` is null until they have been read; `closeToTray` is null until chosen, and counts as on.
 */
export function NotificationsView({
  settings,
  closeToTray = null,
  link = null,
  error,
  onChange,
  onCloseToTray,
  onLink,
}: { settings: NotifySettings | null; closeToTray?: boolean | null; link?: boolean | null; error: string | null } & NotificationsHandlers) {
  const channel = (key: "needs_you" | "done" | "error", part: Partial<NotifyChannel>) => {
    if (settings) onChange({ ...settings, [key]: { ...settings[key], ...part } });
  };
  const quiet = (part: Partial<NotifySettings["quiet_hours"]>) => {
    if (settings) onChange({ ...settings, quiet_hours: { ...settings.quiet_hours, ...part } });
  };
  return (
    <div className="notifications">
      {settings === null && !error && <p className="notifications-dialog__line">Reading your settings…</p>}
      {settings !== null &&
        EVENTS.map(({ key, label }) => (
          <section key={key} className="notifications-dialog__group" aria-label={label}>
            <h3 className="notifications-dialog__name">{label}</h3>
            <Switch label="Banner" checked={settings[key].banner} onChange={(banner) => channel(key, { banner })} />
            <Switch label="Sound" checked={settings[key].sound} onChange={(sound) => channel(key, { sound })} />
          </section>
        ))}
      {settings !== null && (
        <section className="notifications-dialog__group" aria-label="Subagent done">
          <h3 className="notifications-dialog__name">Subagent done</h3>
          <Switch label="Sound" checked={settings.subagent_sound} onChange={(subagent_sound) => onChange({ ...settings, subagent_sound })} reason="A short sound when a subagent finishes." />
        </section>
      )}
      {settings !== null && (
        <section className="notifications-dialog__group" aria-label="Terminal programs">
          <h3 className="notifications-dialog__name">Terminal programs</h3>
          <Switch
            label="Alerts from terminal programs (bell, OSC 9, OSC 777)"
            checked={settings.terminal_alerts}
            onChange={(terminal_alerts) => onChange({ ...settings, terminal_alerts })}
            reason="Off: the row still changes, with no banner or sound."
          />
        </section>
      )}
      {settings !== null && typeof link === "boolean" && (
        <section className="notifications-dialog__group" aria-label="Notification centre">
          <h3 className="notifications-dialog__name">Notification centre</h3>
          <Switch
            label="Open sessions from the notification centre"
            checked={link}
            onChange={(on) => onLink?.(on)}
            reason="Adds a marshell:// link for your Windows account. It can only bring Marshell forward and select a session."
          />
        </section>
      )}
      {settings !== null && (
        <section className="notifications-dialog__group" aria-label="Quiet hours">
          <Switch
            label="Quiet hours"
            checked={settings.quiet_hours.enabled}
            onChange={(enabled) => quiet({ enabled })}
            reason="No sounds or taskbar flashes between these times. Banners still arrive, silently."
          />
          <div className="notifications-dialog__times">
            <label className="notifications-dialog__time">
              <span>From</span>
              <input
                className="notifications-dialog__input"
                type="time"
                value={settings.quiet_hours.start}
                onChange={(e) => e.target.value && quiet({ start: e.target.value })}
              />
            </label>
            <label className="notifications-dialog__time">
              <span>To</span>
              <input
                className="notifications-dialog__input"
                type="time"
                value={settings.quiet_hours.end}
                onChange={(e) => e.target.value && quiet({ end: e.target.value })}
              />
            </label>
          </div>
        </section>
      )}
      {settings !== null && (
        <section className="notifications-dialog__group" aria-label="Close button">
          <h3 className="notifications-dialog__name">Close button</h3>
          <Switch label="Keep Marshell running in the tray when I close the window" checked={closeToTray !== false} onChange={(keep) => onCloseToTray?.(keep)} />
        </section>
      )}
      {error && (
        <p className="notifications-dialog__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

/** The Notifications section on the app's settings: each change is saved as it is made, and the switches show what the core stored. */
export function NotificationsSection({ sync }: { sync?: SettingsSync }) {
  const [settings, setSettings] = useState<NotifySettings | null>(null);
  const [closeToTray, setCloseToTray] = useState<boolean | null>(null);
  const [link, setLink] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void readNotificationLink().then((on) => live && setLink(on));
    sync?.load().then(
      () => {
        if (!live) return;
        setSettings(sync.notifications);
        setCloseToTray(sync.closeToTray);
      },
      () => live && setError(SETTINGS_UNREADABLE),
    );
    return () => {
      live = false;
    };
  }, [sync]);
  const save = async (next: NotifySettings) => {
    if (!sync) return;
    setError(null);
    try {
      await sync.setNotifications(next);
      setSettings(sync.notifications);
    } catch (e) {
      setError(e instanceof SettingsReadError ? e.message : failureText(e));
    }
  };
  const saveCloseToTray = async (keep: boolean) => {
    if (!sync) return;
    setError(null);
    try {
      await sync.setCloseToTray(keep);
      setCloseToTray(sync.closeToTray);
    } catch (e) {
      setError(e instanceof SettingsReadError ? e.message : failureText(e));
    }
  };
  const saveLink = async (on: boolean) => {
    setError(null);
    try {
      setLink(await setNotificationLink(on));
    } catch (e) {
      setError(failureText(e));
    }
  };
  return (
    <NotificationsView
      settings={settings}
      closeToTray={closeToTray}
      link={link}
      error={error}
      onChange={(next) => void save(next)}
      onCloseToTray={(keep) => void saveCloseToTray(keep)}
      onLink={(on) => void saveLink(on)}
    />
  );
}

/** Temporary until the Settings page is wired in (slice 6e1 Task 9). */
export function NotificationsDialog({ sync, onClose }: { sync?: SettingsSync; onClose: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  useDialogFocus(dialog, title, onClose);
  return (
    <div className="notifications-dialog">
      <div className="notifications-dialog__backdrop" onClick={onClose} />
      <div ref={dialog} className="notifications-dialog__sheet" role="dialog" aria-modal="true" aria-label="Notifications">
        <h2 ref={title} tabIndex={-1} className="notifications-dialog__title">
          Notifications
        </h2>
        <div className="notifications-dialog__body">
          <NotificationsSection sync={sync} />
        </div>
        <footer className="notifications-dialog__foot">
          <button type="button" className="notifications-dialog__btn" onClick={onClose}>
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}
