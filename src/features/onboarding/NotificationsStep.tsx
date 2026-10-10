import type { Os } from "../../lib/keymap";
import "./Onboarding.css";

export type NotificationPermission = "not-asked" | "granted" | "denied";

export const SOUNDS = ["Needs you", "Escalation", "Done", "Subagent done", "Error"] as const;

const OS_WORD: Record<Os, string> = { windows: "Windows", mac: "macOS", linux: "your desktop" };
const SETTINGS_PATH: Record<Os, string> = {
  windows: "Settings → System → Notifications",
  mac: "System Settings → Notifications",
  linux: "your desktop's notification settings",
};

/** Screen 3: ask the OS once, preview each sound, and say what still works if banners are blocked. Never announces itself. */
export function NotificationsStep({ permission, os }: { permission: NotificationPermission; os: Os }) {
  const word = OS_WORD[os];
  return (
    <section className="notify-step" aria-label="Notifications">
      {permission === "not-asked" && (
        <p className="notify-step__ask">
          Marshell can show a banner when a session needs you. <button type="button" className="notify-step__allow">Ask {word} to allow notifications</button>
        </p>
      )}
      {permission === "granted" && <p className="notify-step__state">Allowed. A banner never takes focus from what you are typing in.</p>}
      {permission === "denied" && (
        <p className="notify-step__state" data-problem="">
          {word === "your desktop" ? "Your desktop is" : `${word} is`} blocking notifications for Marshell. Turn them on in {SETTINGS_PATH[os]}; the badge and the lane still work.
        </p>
      )}
      <ul className="notify-step__sounds" aria-label="Sounds">
        {SOUNDS.map((s) => (
          <li key={s} className="notify-step__sound">
            <span className="notify-step__sound-name">{s}</span>
            <button type="button" className="notify-step__play">
              <span aria-hidden="true">▶</span> Play sound: {s}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
