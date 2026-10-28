import { useEffect, useId, useRef, useState } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { Chip } from "../../components/Chip/Chip";
import { ContextMenu, type MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { Conversation } from "../../generated/Conversation";
import type { HistoryPatch } from "../../generated/HistoryPatch";
import type { TurnItem } from "../../generated/TurnItem";
import { filesChanged } from "./changedFiles";
import { FilesChanged } from "./FilesChanged";
import { MarkdownText, type MarkdownTextProps } from "./MarkdownText";
import { ToolCard } from "./ToolCard";
import "./History.css";

/** For a conversation Marshell never archived. The view that fetches it shows this when the core answers 404. */
export const NO_ARCHIVE_COPY = "Marshell has no copy of this conversation yet.";

/** The agent's name in the view's sentences; an agent not named here reads as Claude Code, the one History had before Codex. */
const agentName = (agent: string) => (agent === "codex" ? "Codex" : "Claude Code");

const limitedNotice = (agent: string, version: string | undefined) =>
  version
    ? `Status limited: ${agentName(agent)} ${version} saved this conversation, and Marshell hasn't been tested with that version. Some parts may be missing.`
    : `Status limited: this conversation has no ${agentName(agent)} version, so some parts may be missing.`;

/** The conversation view's tabs, in order. */
export type ConversationTab = "conversation" | "recording" | "files";
const TABS: ConversationTab[] = ["conversation", "recording", "files"];

/** The tab a key moves to from `current` (ArrowLeft and ArrowRight wrap, Home and End jump), or null for any other key. */
export function tabKey(current: ConversationTab, key: string): ConversationTab | null {
  const i = TABS.indexOf(current);
  if (key === "ArrowRight") return TABS[(i + 1) % TABS.length]!;
  if (key === "ArrowLeft") return TABS[(i + TABS.length - 1) % TABS.length]!;
  if (key === "Home") return TABS[0]!;
  if (key === "End") return TABS[TABS.length - 1]!;
  return null;
}

/** Which inline editor is open in the header. */
export type ConversationEditor = "rename" | "tags" | "delete" | "resume";

export type ConversationViewProps = {
  conversation: Conversation;
  /** The turn to bring into view, such as the one a search hit is in. */
  scrollToTurn?: number;
  onResume: () => void;
  /** The question to ask before resuming (a conversation that may still be open elsewhere), or null to resume at once. */
  resumeWarning?: string | null;
  /** Called with the tab that already holds this conversation. */
  onGoTo: (tab: string) => void;
  /** The tab that holds this conversation right now, or null when none does. */
  liveTab: string | null;
  /** A file or web link clicked in assistant text. Without it a click does nothing. */
  onLink?: MarkdownTextProps["onLink"];
  /** Starts a new conversation from this one. */
  onFork: () => void;
  /** Changes the supplied marks. The caller closes the editor when the change was accepted. */
  onPatch: (p: HistoryPatch) => void;
  /** Marks or unmarks a turn (0-based). */
  onBookmark: (turn: number, on: boolean) => void;
  onExport: () => void;
  onCopyMarkdown: () => void;
  onCopyResume: () => void;
  onOpenOutside: () => void;
  onDelete: () => void;
  /** The open editor, or null. */
  editor: ConversationEditor | null;
  onEditor: (e: ConversationEditor | null) => void;
  /** The core's reason for refusing the last change, shown under the open editor. */
  error?: string;
  /** The open tab. Without it the view shows the conversation. */
  tab?: ConversationTab;
  onTab?: (t: ConversationTab) => void;
  /** The turns (0-based) that "Play from here" can start: the ones inside the recording. */
  playableTurns?: ReadonlySet<number>;
  /** Plays the recording from this turn (0-based); the owner switches to the Recording tab. */
  onPlayFrom?: (turn: number) => void;
  /** The Recording panel, which the owner builds. */
  recording?: React.ReactNode;
};

const NO_TURNS: ReadonlySet<number> = new Set();

/** The "More actions" menu, in order. Pin and Archive turn into Unpin and Unarchive when set. */
export function moreActions(marks: { pinned: boolean; archived: boolean }): MenuItem[] {
  return [
    { id: "rename", label: "Rename…" },
    { id: "tags", label: "Tags…" },
    { id: "pin", label: marks.pinned ? "Unpin" : "Pin" },
    { id: "archive", label: marks.archived ? "Unarchive" : "Archive" },
    { id: "export", label: "Export…" },
    { id: "copy", label: "Copy as markdown" },
    { id: "copy-resume", label: "Copy resume command" },
    { id: "open-outside", label: "Open in system terminal" },
    { id: "delete", label: "Delete Marshell's copy…" },
  ];
}

const parseTags = (text: string) =>
  text
    .split(",")
    .map((t) => t.trim())
    .filter((t) => t !== "");

function Item({ item, cwd, onLink }: { item: TurnItem; cwd: string; onLink: MarkdownTextProps["onLink"] }) {
  switch (item.kind) {
    case "text":
      return <MarkdownText markdown={item.markdown} cwd={cwd} onLink={onLink} />;
    case "tool":
      return <ToolCard card={item.card} />;
    case "subagent":
      return (
        <details className="tool">
          <summary className="tool__summary">{`Subagent: ${item.description}`}</summary>
          <p className="tool__label">Prompt</p>
          <pre className="tool__detail">{item.prompt}</pre>
          {item.result ? (
            <>
              <p className="tool__label">Result</p>
              <pre className="tool__detail">{item.result}</pre>
            </>
          ) : null}
        </details>
      );
    case "compacted":
      return (
        <div className="compacted">
          <hr />
          <p>Conversation compacted</p>
        </div>
      );
  }
}

const when = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

/** One archived conversation: its header with Resume, Fork and More actions, then each prompt with what the agent did in answer. Transcript text and the person's names and tags are untrusted, so only assistant text goes through the sanitiser; everything else is text. */
export function ConversationView({
  conversation: c,
  scrollToTurn,
  onResume,
  resumeWarning = null,
  onGoTo,
  liveTab,
  onLink,
  onFork,
  onPatch,
  onBookmark,
  onExport,
  onCopyMarkdown,
  onCopyResume,
  onOpenOutside,
  onDelete,
  editor,
  onEditor,
  error,
  tab = "conversation",
  onTab = () => {},
  playableTurns = NO_TURNS,
  onPlayFrom = () => {},
  recording = null,
}: ConversationViewProps) {
  const uid = useId();
  const root = useRef<HTMLElement>(null);
  const moreRef = useRef<HTMLButtonElement>(null);
  const boxRef = useRef<HTMLInputElement>(null);
  const deleteRef = useRef<HTMLButtonElement>(null);
  const resumeRef = useRef<HTMLButtonElement>(null);
  const cancelResumeRef = useRef<HTMLButtonElement>(null);
  const shownEditor = useRef<ConversationEditor | null>(null);
  const tabRefs = useRef<Partial<Record<ConversationTab, HTMLButtonElement | null>>>({});
  const pendingTurn = useRef<number | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  // Keyed on the conversation's identity, not `c`: a refresh after a bookmark, pin or rename replaces `c`, and the view must stay where the person is.
  useEffect(() => {
    if (scrollToTurn === undefined) return;
    root.current?.querySelector(`[data-turn="${scrollToTurn}"]`)?.scrollIntoView?.({ block: "start" });
  }, [c.agent, c.session_id, scrollToTurn]);

  // "Go to turn" from the Files changed tab: the turns exist once the Conversation tab is showing. The button that was pressed
  // is gone with its tab, so focus goes to the turn's prompt.
  useEffect(() => {
    if (tab !== "conversation" || pendingTurn.current === null) return;
    const turn = root.current?.querySelector(`[data-turn="${pendingTurn.current}"]`);
    turn?.scrollIntoView?.({ block: "start" });
    turn?.querySelector<HTMLElement>(".turn__prompt")?.focus({ preventScroll: true });
    pendingTurn.current = null;
  }, [tab]);

  // An editor takes focus when it opens; when it closes, focus goes back to "More actions".
  useEffect(() => {
    // The resume question focuses Cancel first, as the restart bar does, so a stray Enter resumes nothing; closing it returns to Resume.
    if (editor !== null) (editor === "delete" ? deleteRef : editor === "resume" ? cancelResumeRef : boxRef).current?.focus();
    else if (shownEditor.current === "resume") (resumeRef.current ?? moreRef.current)?.focus();
    else if (shownEditor.current !== null) moreRef.current?.focus();
    shownEditor.current = editor;
  }, [editor]);

  const meta = [c.cwd, c.branch, c.model].filter(Boolean).join(" · ");
  const title = c.title || "Untitled conversation";
  const errorId = `${uid}-error`;
  const hintId = `${uid}-hint`;

  const fileCount = filesChanged(c).length;
  const labels: Record<ConversationTab, string> = {
    conversation: "Conversation",
    recording: "Recording",
    files: fileCount > 0 ? `Files changed (${fileCount})` : "Files changed",
  };
  const tabId = (t: ConversationTab) => `${uid}-tab-${t}`;
  const panelId = (t: ConversationTab) => `${uid}-panel-${t}`;
  const onTabKey = (e: React.KeyboardEvent) => {
    const next = tabKey(tab, e.key);
    if (next === null) return;
    e.preventDefault();
    onTab(next);
    tabRefs.current[next]?.focus();
  };
  // Esc inside the player first returns to the Recording tab; the key is handled here, so it doesn't also close the view.
  const recordingKey = (e: React.KeyboardEvent) => {
    if (e.key !== "Escape" || !(e.target instanceof Element) || !e.target.closest(".player")) return;
    e.preventDefault();
    tabRefs.current.recording?.focus();
  };

  const openMenu =() => {
    const r = moreRef.current?.getBoundingClientRect();
    setMenu({ x: r?.left ?? 0, y: r?.bottom ?? 0 });
  };
  const run = (id: string) => {
    setMenu(null);
    if (id === "rename" || id === "tags" || id === "delete") return onEditor(id);
    // The action finishes elsewhere (a dialog, a refresh), so focus goes back to the button that opened the menu first.
    moreRef.current?.focus();
    if (id === "pin") onPatch({ pinned: !c.marks.pinned });
    else if (id === "archive") onPatch({ archived: !c.marks.archived });
    else if (id === "export") onExport();
    else if (id === "copy") onCopyMarkdown();
    else if (id === "copy-resume") onCopyResume();
    else if (id === "open-outside") onOpenOutside();
  };

  // Enter saves and Esc cancels; the key goes no further, so Esc doesn't also close the conversation.
  const boxKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      e.preventDefault();
      onEditor(null);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const value = e.currentTarget.value;
      onPatch(editor === "tags" ? { tags: parseTags(value) } : { name: value });
    }
  };
  const confirmKey = (e: React.KeyboardEvent) => {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    e.preventDefault();
    onEditor(null);
  };

  return (
    <article ref={root} className="conversation" aria-label={title}>
      <header className="conversation__head">
        <div className="conversation__titles">
          <h2 className="conversation__title" dir="auto">
            {title}
          </h2>
          <p className="conversation__meta" dir="auto">
            {meta}
          </p>
          {(c.marks.archived || c.marks.tags.length > 0) && (
            <p className="history__chips conversation__chips">
              {c.marks.archived && <Chip>Archived</Chip>}
              {c.marks.tags.map((tag) => (
                <Chip key={tag}>{tag}</Chip>
              ))}
            </p>
          )}
        </div>
        <div className="conversation__actions">
          {liveTab !== null ? (
            <button type="button" className="conversation__action" onClick={() => onGoTo(liveTab)}>
              Go to session
            </button>
          ) : (
            <button ref={resumeRef} type="button" className="conversation__action" onClick={() => (resumeWarning ? onEditor("resume") : onResume())}>
              Resume
            </button>
          )}
          {c.agent !== "codex" && (
            <button type="button" className="conversation__action conversation__action--quiet" onClick={onFork}>
              Fork
            </button>
          )}
          <button
            ref={moreRef}
            type="button"
            className="conversation__action conversation__action--quiet"
            aria-haspopup="menu"
            aria-expanded={menu !== null}
            onClick={openMenu}
          >
            More actions
          </button>
        </div>
      </header>
      {menu && (
        <ContextMenu
          items={moreActions(c.marks)}
          label="More actions"
          anchor={menu}
          onRun={run}
          onClose={({ refocus }) => {
            setMenu(null);
            if (refocus) moreRef.current?.focus();
          }}
        />
      )}
      {(editor === "rename" || editor === "tags") && (
        <div className="conversation__editor">
          <label className="conversation__label" htmlFor={`${uid}-box`}>
            {editor === "rename" ? "Conversation name" : "Tags, separated by commas"}
          </label>
          <input
            key={editor}
            ref={boxRef}
            id={`${uid}-box`}
            type="text"
            className="conversation__input"
            dir="auto"
            autoComplete="off"
            spellCheck={false}
            defaultValue={editor === "rename" ? (c.marks.name ?? "") : c.marks.tags.join(", ")}
            aria-invalid={error ? true : undefined}
            aria-describedby={[editor === "rename" ? hintId : null, error ? errorId : null].filter(Boolean).join(" ") || undefined}
            onKeyDown={boxKey}
          />
          {editor === "rename" && (
            <p id={hintId} className="conversation__hint">
              {c.agent === "codex" ? "Leave empty to use the first prompt." : "Leave empty to use Claude Code's title."}
            </p>
          )}
          {error && (
            <p className="conversation__error" id={errorId}>
              {error}
            </p>
          )}
        </div>
      )}
      {editor === "delete" && (
        <div className="conversation__editor" onKeyDown={confirmKey}>
          <p className="conversation__confirm" id={`${uid}-confirm`} dir="auto">
            {`Delete Marshell's copy of “${title}”? ${agentName(c.agent)}'s own file is not touched.`}
          </p>
          {error && (
            <p className="conversation__error" id={errorId}>
              {error}
            </p>
          )}
          <div className="conversation__actions">
            <button
              ref={deleteRef}
              type="button"
              className="conversation__action conversation__action--danger"
              aria-describedby={`${uid}-confirm`}
              onClick={onDelete}
            >
              Delete
            </button>
            <button type="button" className="conversation__action conversation__action--quiet" onClick={() => onEditor(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}
      {editor === "resume" && resumeWarning && (
        <div className="conversation__editor" onKeyDown={confirmKey}>
          <p className="conversation__confirm" id={`${uid}-resume`} dir="auto">
            {resumeWarning}
          </p>
          <div className="conversation__actions">
            <button ref={cancelResumeRef} type="button" className="conversation__action" aria-describedby={`${uid}-resume`} onClick={() => onEditor(null)}>
              Cancel
            </button>
            <button
              type="button"
              className="conversation__action conversation__action--quiet"
              onClick={() => {
                onEditor(null);
                onResume();
              }}
            >
              Resume here
            </button>
          </div>
        </div>
      )}
      {c.limited && (
        <p className="conversation__notice" role="note">
          {limitedNotice(c.agent, c.version)}
        </p>
      )}
      <div className="conversation__tabs" role="tablist" aria-label="Conversation views" onKeyDown={onTabKey}>
        {TABS.map((t) => (
          <button
            key={t}
            ref={(el) => {
              tabRefs.current[t] = el;
            }}
            type="button"
            role="tab"
            id={tabId(t)}
            aria-controls={panelId(t)}
            tabIndex={tab === t ? 0 : -1}
            {...selectedProps(tab === t, "selected", "conversation__tab")}
            onClick={() => onTab(t)}
          >
            {labels[t]}
          </button>
        ))}
      </div>
      <div role="tabpanel" id={panelId("conversation")} aria-labelledby={tabId("conversation")} hidden={tab !== "conversation"}>
        {tab === "conversation" &&
          c.turns.map((t) => {
            const marked = c.marks.bookmarked_turns.includes(t.index);
            return (
              <section
                key={t.index}
                className="turn"
                data-turn={t.index}
                data-target={t.index === scrollToTurn ? "true" : undefined}
                aria-labelledby={`${uid}-turn-${t.index}`}
              >
                <h3 className="turn__prompt" id={`${uid}-turn-${t.index}`} dir="auto" tabIndex={-1}>
                  {t.prompt}
                </h3>
                <div className="turn__meta">
                  {t.at_ms > 0 && (
                    <time className="turn__time" dateTime={new Date(t.at_ms).toISOString()}>
                      {when(t.at_ms)}
                    </time>
                  )}
                  {playableTurns.has(t.index) && (
                    <button type="button" className="turn__bookmark" aria-label={`Play turn ${t.index + 1} from here`}
                      onClick={() => {
                        // The button goes with the Conversation tab; focus goes to the Recording tab, as Esc in the player does.
                        onPlayFrom(t.index);
                        tabRefs.current.recording?.focus();
                      }}
                    >
                      Play from here
                    </button>
                  )}
                  <button
                    type="button"
                    className="turn__bookmark"
                    aria-label={`Bookmark turn ${t.index + 1}`}
                    aria-pressed={marked}
                    onClick={() => onBookmark(t.index, !marked)}
                  >
                    {marked ? "Bookmarked" : "Bookmark"}
                  </button>
                </div>
                {t.items.length > 0 && (
                  <div className="turn__items">
                    {t.items.map((item, i) => (
                      <Item key={i} item={item} cwd={c.cwd} onLink={onLink} />
                    ))}
                  </div>
                )}
              </section>
            );
          })}
      </div>
      <div role="tabpanel" id={panelId("recording")} aria-labelledby={tabId("recording")} hidden={tab !== "recording"} onKeyDown={recordingKey}>
        {tab === "recording" && recording}
      </div>
      <div role="tabpanel" id={panelId("files")} aria-labelledby={tabId("files")} hidden={tab !== "files"}>
        {tab === "files" && (
          <FilesChanged
            conversation={c}
            onGoToTurn={(turn) => {
              pendingTurn.current = turn;
              onTab("conversation");
            }}
          />
        )}
      </div>
    </article>
  );
}
