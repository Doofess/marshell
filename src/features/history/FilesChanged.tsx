import { DiffView } from "../../components/DiffView/DiffView";
import type { Conversation } from "../../generated/Conversation";
import { editDiff, filesChanged } from "./changedFiles";

/** The note and the empty line, per agent: each names the tools whose edits the transcript records. */
const CLAUDE_COPY = {
  note: "Only edits made with Claude Code's Edit and Write tools are listed. Changes made through shell commands, such as sed or a code generator, aren't.",
  none: "Claude Code didn't edit any files in this conversation.",
};
const COPY: Record<string, { note: string; none: string }> = {
  claude: CLAUDE_COPY,
  codex: {
    note: "Only edits made with Codex's apply_patch are listed. Changes made through shell commands, such as sed or a code generator, aren't.",
    none: "Codex didn't edit any files in this conversation.",
  },
};

/** `+3 −1`, with an en dash for a count the transcript didn't give. */
const counts = (added: number | null, removed: number | null) => `+${added ?? "–"} −${removed ?? "–"}`;

/**
 * The files the agent edited, per file, each edit with its diff and a button back to its turn. Paths and diff lines render as
 * text. `turn` counts from 0 (as `Turn.index` does), so the visible number and the button's label are `turn + 1`.
 */
export function FilesChanged({ conversation, onGoToTurn }: { conversation: Conversation; onGoToTurn: (turn: number) => void }) {
  const files = filesChanged(conversation);
  const copy = COPY[conversation.agent] ?? CLAUDE_COPY;
  return (
    <section className="files" aria-label="Files changed">
      <h3 className="files__heading">{files.length === 1 ? "1 file changed" : `${files.length} files changed`}</h3>
      <p className="files__note">{copy.note}</p>
      {files.length === 0 ? (
        <p className="files__empty">{copy.none}</p>
      ) : (
        <ul className="files__list">
          {files.map((f) => (
            <li key={f.path} className="file">
              <h4 className="file__head">
                <span className="file__path" dir="auto">
                  {f.path}
                </span>
                <span className="file__count">{counts(f.added, f.removed)}</span>
                {f.created && <span className="file__new">New file</span>}
              </h4>
              <ul className="file__edits">
                {f.edits.map((e, i) => (
                  <li key={i} className="file__edit">
                    <div className="file__edit-head">
                      <span>Turn {e.turn + 1}</span>
                      <button type="button" className="file__goto" aria-label={`Go to turn ${e.turn + 1}`} onClick={() => onGoToTurn(e.turn)}>
                        Go to turn
                      </button>
                    </div>
                    {e.card.detail && <DiffView diff={editDiff(f.path, e.card)} label={`Turn ${e.turn + 1}, ${f.path}`} />}
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
