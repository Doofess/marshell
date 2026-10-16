/** The text input xterm keeps for the shell panel's terminal (a session's terminal has one too, but not inside `.shell-panel`). */
export const SHELL_TERMINAL_INPUT = ".shell-panel .xterm-helper-textarea";
/** How long to wait for a terminal that is still starting (the shell is requested, the font loads) before giving up. */
const WAIT_MS = 3000;

/** True when the keyboard focus is in the shell panel's terminal: not its header buttons, and not a session's terminal. */
export function shellTerminalFocused(doc: Pick<Document, "activeElement">): boolean {
  const el = doc.activeElement;
  return el !== null && el.classList.contains("xterm-helper-textarea") && el.closest(".shell-panel") !== null;
}

/**
 * Puts the keyboard focus in the shell panel's terminal. The terminal may not exist yet (the panel was just shown, so the
 * shell is still being asked for), so it looks again every frame for a few seconds.
 */
export function focusShellTerminal(
  doc: Pick<Document, "querySelector">,
  frame: (callback: () => void) => void = (f) => void requestAnimationFrame(f),
  now: () => number = () => performance.now(),
): void {
  const until = now() + WAIT_MS;
  const attempt = () => {
    const input = doc.querySelector<HTMLElement>(SHELL_TERMINAL_INPUT);
    if (input) input.focus();
    else if (now() < until) frame(attempt);
  };
  attempt();
}
