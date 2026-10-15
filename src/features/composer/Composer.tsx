import { AgentMark } from "../../components/AgentMark/AgentMark";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
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

const prompt = (
  <svg className="composer__prompt" width="12" height="16" viewBox="0 0 12 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3.5 3.5L8.5 8l-5 4.5" />
  </svg>
);

export type ComposerVariant = "full" | "compact";

export type ComposerProps = {
  /** The session this prompt writes to; null for an empty pane. */
  row: RowModel | null;
  value?: string;
  os?: Os;
  /** `full` under the terminal of a single-session window; `compact` inside each pane in split view. */
  variant?: ComposerVariant;
  /** Whether this is the focused prompt, which gets the vendor-coloured edge. Off by default for both: the edge follows real focus (:focus-within), and a pane's compact line is told when its pane is the active one. */
  focused?: boolean;
  /** Full variant, split view: which pane this is, so the target is never a guess. */
  pane?: { index: number; count: number };
};

/**
 * The prompt line, set like a command line: a flat bar docked to the edge of its terminal, a prompt glyph in the session's
 * colour, the field in the terminal's own mono font, and Send with the Enter key drawn on it. `full` adds a quiet row of
 * tools under the line; `compact` is the line alone, for one inside every pane. The height never changes with the
 * session's state (notices sit in the row), so the terminal above never resizes. The facts about the session are not here:
 * they are the status bar's.
 */
export function Composer({ row, value = "", os = "windows", variant = "full", focused = false, pane }: ComposerProps) {
  const focusKeys = shortcutLabel(commandById("focus-prompt").binding!, os);
  const state: ComposerState = row ? composerState(row, value) : "waiting";
  const blocked = row === null || state === "waiting" || state === "ended";
  const canSend = !blocked && (state === "typing" || (state === "working" && value.trim() !== ""));
  const name = row?.name ?? "this pane";
  const caution = row ? modeCaution(row.mode) : false;
  const inSplit = pane !== undefined && pane.count > 1;
  const compact = variant === "compact";

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
      ? null // the button already says Queue; no extra words
      : state === "waiting"
        ? { glyph: <StatusGlyph kind={row.status} />, text: `${row.name} ${row.status === "needs-question" ? "has a question" : "is waiting for your approval"}` }
        : state === "ended"
          ? { glyph: <StatusGlyph kind="ended" />, text: `${row.name} has ended` }
          : null;

  const noticeEl = notice && (
    <p className="composer__notice" title={notice.text}>
      {notice.glyph}
      <span>{notice.text}</span>
    </p>
  );
  const resume =
    state === "ended" ? (
      <button type="button" className="composer__resume">
        Resume
      </button>
    ) : null;
  // A compact prompt for an ended session offers the way back instead of a Send that can never work.
  const showSend = !(compact && state === "ended");

  return (
    <form className="composer" aria-label={`Write to ${name}`} data-agent={row?.agent} data-state={row ? state : "empty-pane"} data-variant={variant} data-focused={focused || undefined}>
      <div className="composer__line">
        {prompt}
        <textarea
          className="composer__field"
          rows={1}
          aria-label={`Message ${name}`}
          placeholder={placeholder}
          title={`Write a prompt (${focusKeys}). Enter sends, Shift+Enter adds a line.`}
          disabled={blocked}
          defaultValue={value}
        />
        {compact && noticeEl}
        {compact && resume}
        {showSend && (
          <button type="submit" className="composer__send" aria-disabled={!canSend} title={canSend ? undefined : "Write something to send"}>
            <span>{state === "working" ? "Queue" : "Send"}</span>
            <kbd className="composer__kbd" aria-hidden="true">
              ↵
            </kbd>
          </button>
        )}
      </div>

      {!compact && (
        <div className="composer__tools">
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
            <span className="composer__mode" data-tone={caution ? "caution" : undefined}>
              {caution && <AuxGlyph kind="caution" />}
              {modeLabel(row.mode)}
            </span>
          )}
          <button type="button" className="composer__tool" title="Slash commands (/)" disabled={blocked}>
            <span className="composer__tool-key">/</span> <span>commands</span>
          </button>
          <button type="button" className="composer__tool" title="Mention a file (@)" disabled={blocked}>
            <span className="composer__tool-key">@</span> <span>files</span>
          </button>
          <button type="button" className="composer__tool" title="Attach an image" disabled={blocked}>
            <span>attach</span>
          </button>
          <span className="composer__spacer" />
          {noticeEl ?? <span className="composer__hint">Enter to send · Shift+Enter for a new line</span>}
          {resume}
        </div>
      )}
    </form>
  );
}
