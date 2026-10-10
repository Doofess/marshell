import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Chip } from "../../components/Chip/Chip";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { modeCaution } from "../sidebar/format";
import type { RowModel } from "../sidebar/types";
import { modeLabel } from "../window/SessionHeader";
import "./Composer.css";

export type ComposerState = "empty" | "typing" | "working" | "waiting" | "ended";

/**
 * What the composer can do for this session right now. A session waiting on the user is blocked on purpose: the CLI would
 * read a typed line as the answer to its question, so the answer has to be given on the request, never by accident here.
 */
export function composerState(row: RowModel, value: string): ComposerState {
  if (row.status === "needs-permission" || row.status === "needs-question") return "waiting";
  if (row.status === "ended") return "ended";
  if (row.status === "working") return "working";
  return value.trim() ? "typing" : "empty";
}

const slash = <span aria-hidden="true">/</span>;
const at = <span aria-hidden="true">@</span>;
const clip = (
  <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
    <path d="M11.2 6.6L7 10.8a2.4 2.4 0 0 1-3.4-3.4l4.6-4.6a1.6 1.6 0 0 1 2.3 2.3L6 9.6a.8.8 0 0 1-1.1-1.1l3.9-3.9" />
  </svg>
);
const arrow = (
  <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
    <path d="M7 11.5v-9M3 6.5l4-4 4 4" />
  </svg>
);

export type ComposerProps = {
  /** The session of the focused pane; null when the focused pane is empty. */
  row: RowModel | null;
  value?: string;
  os?: Os;
  /** Set in split view: which pane has the focus, so the target is never a guess. */
  pane?: { index: number; count: number };
};

/**
 * The prompt line: one field docked under the terminals that always writes to the focused pane's session. It is the
 * obvious place for a longer prompt; the terminal itself stays typeable. The accent is the session's vendor colour.
 * Its height never changes with the session's state (notices live in the bar row), so the terminals above never resize.
 */
export function Composer({ row, value = "", os = "windows", pane }: ComposerProps) {
  const focusKeys = shortcutLabel(commandById("focus-prompt").binding!, os);
  const state: ComposerState = row ? composerState(row, value) : "waiting";
  const blocked = row === null || state === "waiting" || state === "ended";
  const canSend = !blocked && (state === "typing" || (state === "working" && value.trim() !== ""));
  const name = row?.name ?? "this pane";
  const caution = row ? modeCaution(row.mode) : false;
  const inSplit = pane !== undefined && pane.count > 1;

  const placeholder = !row
    ? "Choose a session for this pane first"
    : state === "waiting"
      ? row.status === "needs-question"
        ? "Answer the question in the lane, or in the terminal"
        : "Answer the request in the lane, or in the terminal"
      : state === "ended"
        ? "This session has ended"
        : `Message ${row.name}…`;

  const notice = !row
    ? { glyph: null, text: "This pane is empty" }
    : state === "working"
      ? { glyph: <AgentMark agent={row.agent} size={16} working />, text: `${row.name} is working · your message is queued` }
      : state === "waiting"
        ? { glyph: <StatusGlyph kind={row.status} />, text: `${row.name} ${row.status === "needs-question" ? "has a question" : "is waiting for your approval"}` }
        : state === "ended"
          ? { glyph: <StatusGlyph kind="ended" />, text: `${row.name} has ended` }
          : null;

  return (
    <form className="composer" aria-label={`Write to ${name}`} data-agent={row?.agent} data-state={row ? state : "empty-pane"}>
      <textarea
        className="composer__field"
        rows={1}
        aria-label={`Message ${name}`}
        placeholder={placeholder}
        title={`Write a prompt (${focusKeys})`}
        disabled={blocked}
        defaultValue={value}
      />

      <div className="composer__bar">
        {row && (
          <span className="composer__target" title={inSplit ? `Sends to the focused pane, ${row.name}` : `Sends to ${row.name}`}>
            <AgentMark agent={row.agent} size={16} />
            <span className="composer__target-name" dir="auto">
              {row.name}
            </span>
            {inSplit && (
              <span className="composer__pane">
                Pane {pane.index + 1} of {pane.count}
              </span>
            )}
          </span>
        )}
        {row?.mode && (
          <span className="composer__mode">
            <Chip tone={caution ? "caution" : "neutral"}>{modeLabel(row.mode)}</Chip>
          </span>
        )}
        <button type="button" className="composer__tool" aria-label="Slash commands" title="Slash commands (/)" disabled={blocked}>
          {slash}
        </button>
        <button type="button" className="composer__tool" aria-label="Mention a file" title="Mention a file (@)" disabled={blocked}>
          {at}
        </button>
        <button type="button" className="composer__tool" aria-label="Attach an image" title="Attach an image" disabled={blocked}>
          {clip}
        </button>
        <span className="composer__spacer" />
        {notice ? (
          <p className="composer__notice" role="status" title={notice.text}>
            {notice.glyph}
            <span>{notice.text}</span>
          </p>
        ) : (
          <span className="composer__hint">Enter to send · Shift+Enter for a new line</span>
        )}
        {state === "ended" && (
          <button type="button" className="composer__resume">
            Resume
          </button>
        )}
        <button type="submit" className="composer__send" aria-disabled={!canSend} title={canSend ? undefined : "Write something to send"}>
          {arrow}
          <span>{state === "working" ? "Queue" : "Send"}</span>
          <kbd className="composer__kbd" aria-hidden="true">
            ↵
          </kbd>
        </button>
      </div>
    </form>
  );
}
