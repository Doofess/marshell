/** Why Review is off and comments are copied for the shell panel's Changes. */
export const SHELL_CHANGES_NOTE = "The shell panel runs no agent, so Review is off and comments are copied, not sent.";

/** The note, drawn first in the panel's strip: one paragraph, read in order, no live region (it is not news). */
export function ShellNote({ text }: { text: string }) {
  return <p className="changes__notice">{text}</p>;
}
