import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import { SHELL_SIZE, shellSession, type ShellName } from "../terminal/scriptedShell";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import "./ShellPanel.css";

/** The operating system's own default shell, which is what a plain command prompt means to the person using it. */
export const shellNameFor = (os: Os): ShellName => (os === "mac" ? "zsh" : os === "linux" ? "bash" : "PowerShell");

const icon = (d: string) => (
  <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);
const PROMPT = "M2 3.5L5 6L2 8.5M6.5 9h3.5";
const SPLIT = "M1.5 2.5h9v7h-9zM6 2.5v7";
const CHEVRON_DOWN = "M3 4.5L6 7.5L9 4.5";
const CHEVRON_UP = "M3 7.5L6 4.5L9 7.5";

export type ShellPanelProps = {
  /** The panel's whole height, from `shellLayout`. */
  height: number;
  /** The folder the shell sits in: the focused session's project. */
  cwd?: string;
  shellName?: ShellName;
  /** Header only. */
  collapsed?: boolean;
  /** The panel folded itself away because the list needed the room; the user did not ask for it. */
  auto?: boolean;
  os?: Os;
  setting?: TerminalThemeSetting;
};

/**
 * A plain command prompt in the sidebar, for the commands a person still runs by hand. It belongs to no agent: no vendor
 * stripe or accent, no prompt bar, and nothing typed here ever reaches a session. It can open in a pane, and it can hide.
 */
export function ShellPanel({ height, cwd, shellName = "PowerShell", collapsed = false, auto = false, os = "windows", setting = "follow-app" }: ShellPanelProps) {
  const keys = shortcutLabel(commandById("shell-panel").binding!, os);
  return (
    <section className="shell-panel" aria-label="Shell" style={{ blockSize: height }} data-collapsed={collapsed || undefined} data-auto={(collapsed && auto) || undefined}>
      <header className="shell-panel__header">
        <span className="shell-panel__icon">{icon(PROMPT)}</span>
        <span className="shell-panel__name">{shellName}</span>
        {cwd && (
          <span className="shell-panel__cwd" dir="auto">
            {cwd}
          </span>
        )}
        <span className="shell-panel__spacer" />
        <button type="button" className="shell-panel__button" aria-label={`Open ${shellName} in a pane`} title={`Open ${shellName} in a pane`}>
          {icon(SPLIT)}
        </button>
        <button
          type="button"
          className="shell-panel__button"
          aria-label="Shell panel"
          aria-expanded={!collapsed}
          title={`${collapsed ? "Show" : "Hide"} the shell (${keys})`}
        >
          {icon(collapsed ? CHEVRON_UP : CHEVRON_DOWN)}
        </button>
      </header>
      {!collapsed && <ScriptedTerminal setting={setting} label={`${shellName} terminal`} script={shellSession({ shell: shellName, cwd })} size={SHELL_SIZE} />}
    </section>
  );
}
