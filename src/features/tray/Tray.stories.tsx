import type { Meta, StoryObj } from "@storybook/react-vite";
import { Logo } from "../../components/Logo/Logo";
import { ThemePair } from "../../design/ThemePair";
import { TrayIcon } from "./TrayIcon";
import { TrayMenu } from "./TrayMenu";
import { badgeFor, trayTooltip, type TraySession, type TrayState } from "./trayItems";
import "./Tray.css";

const meta: Meta = { title: "Tray and icon", parameters: { layout: "fullscreen" } };
export default meta;

const wait = (number: number, name: string, status: TraySession["status"] = "needs-permission"): TraySession => ({
  number,
  name,
  status,
  phrase: status === "needs-permission" ? "Wants to run `npm test`" : "Has a question",
  agent: "claude",
});
const TWO: TraySession[] = [wait(3, "api-server"), wait(2, "infra", "needs-question")];
const state = (over: Partial<TrayState> = {}): TrayState => ({ os: "windows", sessions: TWO, paused: false, doctorIssues: 4, departures: true, ...over });
const menu = (s: TrayState) => <TrayMenu state={s} caption />;

const panels = (children: (panel: "light" | "dark") => React.ReactNode) => (
  <div style={{ display: "flex", gap: 24, flexWrap: "wrap" }}>
    {(["light", "dark"] as const).map((p) => (
      <div key={p} className="tray-panel" data-panel={p} style={{ gap: 24, display: "inline-flex", alignItems: "center" }}>
        {children(p)}
      </div>
    ))}
  </div>
);

export const IconSizes: StoryObj = {
  render: () => (
    <ThemePair label="The tray icon at 16 and 32 px on a light and a dark panel, idle and with the needs-you dot; and the app icon at 16, 32 and 48" stack>
      {panels((p) => (
        <>
          <TrayIcon size={16} key={`${p}-16`} />
          <TrayIcon size={32} key={`${p}-32`} />
          <TrayIcon size={16} waiting={2} key={`${p}-16w`} />
          <TrayIcon size={32} waiting={2} key={`${p}-32w`} />
          <Logo size={16} />
          <Logo size={32} />
          <Logo size={48} />
        </>
      ))}
    </ThemePair>
  ),
};
export const TrayMenuWindows: StoryObj = { render: () => <ThemePair label="Windows: who needs you, with their numbers, then Departures, pause, the doctor, Settings and quit">{menu(state())}</ThemePair> };
export const TrayMenuNothing: StoryObj = { render: () => <ThemePair label="Nothing waiting: a disabled line says so">{menu(state({ sessions: [] }))}</ThemePair> };
export const TrayMenuMany: StoryObj = { render: () => <ThemePair label="Eight waiting: five are listed and the rest are counted">{menu(state({ sessions: Array.from({ length: 8 }, (_, i) => wait(i + 1, `session-${i + 1}`)) }))}</ThemePair> };
export const TrayMenuPaused: StoryObj = { render: () => <ThemePair label="Paused: the item offers Resume, and the tooltip says so">{menu(state({ paused: true }))}</ThemePair> };
export const TrayMenuMac: StoryObj = { render: () => <ThemePair label="macOS (design only): the same items with the Mac shortcut">{menu(state({ os: "mac" }))}</ThemePair> };
export const TrayMenuLinux: StoryObj = { render: () => <ThemePair label="Linux (design only): the tray is menu-only, so the menu is all there is">{menu(state({ os: "linux" }))}</ThemePair> };

export const Badges: StoryObj = {
  render: () => (
    <ThemePair label="Badges: a Windows overlay icon (1, 2, 9+), the macOS dock badge, and nothing on Linux" stack>
      <table>
        <thead>
          <tr>
            <th scope="col">System</th>
            <th scope="col">Waiting</th>
            <th scope="col">Badge</th>
          </tr>
        </thead>
        <tbody>
          {([["windows", 1], ["windows", 2], ["windows", 12], ["mac", 3], ["linux", 3]] as const).map(([os, n]) => {
            const b = badgeFor(os, n);
            return (
              <tr key={`${os}-${n}`}>
                <td>{os === "mac" ? "macOS" : os === "linux" ? "Linux" : "Windows"}</td>
                <td>{n}</td>
                <td>{b.kind === "none" ? "None (the tray is menu-only)" : `${b.kind === "overlay" ? "Overlay icon" : "Dock badge"}: ${b.text}`}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </ThemePair>
  ),
};
export const Tooltips: StoryObj = {
  render: () => (
    <ThemePair label="The tooltip on the tray icon" stack>
      <ul>
        {[state(), state({ sessions: [wait(1, "a")] }), state({ sessions: [] }), state({ paused: true })].map((s, i) => (
          <li key={i}>{trayTooltip(s)}</li>
        ))}
      </ul>
    </ThemePair>
  ),
};
