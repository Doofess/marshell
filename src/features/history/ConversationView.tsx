import { useEffect, useId, useRef } from "react";
import type { Conversation } from "../../generated/Conversation";
import type { TurnItem } from "../../generated/TurnItem";
import { MarkdownText, type MarkdownTextProps } from "./MarkdownText";
import { ToolCard } from "./ToolCard";
import "./History.css";

/** For a conversation Marshell never archived. The view that fetches it shows this when the core answers 404. */
export const NO_ARCHIVE_COPY = "Marshell has no copy of this conversation yet.";

const limitedNotice = (version: string | undefined) =>
  version
    ? `Status limited: Claude Code ${version} saved this conversation, and Marshell hasn't been tested with that version. Some parts may be missing.`
    : "Status limited: this conversation has no Claude Code version, so some parts may be missing.";

export type ConversationViewProps = {
  conversation: Conversation;
  /** The turn to bring into view, such as the one a search hit is in. */
  scrollToTurn?: number;
  onResume: () => void;
  /** Called with the tab that already holds this conversation. */
  onGoTo: (tab: string) => void;
  /** The tab that holds this conversation right now, or null when none does. */
  liveTab: string | null;
  /** A file or web link clicked in assistant text. Without it a click does nothing. */
  onLink?: MarkdownTextProps["onLink"];
};

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

/** One archived conversation: its header with Resume, then each prompt with what the agent did in answer. Transcript text is untrusted, so only assistant text goes through the sanitiser; everything else is text. */
export function ConversationView({ conversation: c, scrollToTurn, onResume, onGoTo, liveTab, onLink }: ConversationViewProps) {
  const uid = useId();
  const root = useRef<HTMLElement>(null);

  useEffect(() => {
    if (scrollToTurn === undefined) return;
    root.current?.querySelector(`[data-turn="${scrollToTurn}"]`)?.scrollIntoView?.({ block: "start" });
  }, [c, scrollToTurn]);

  const meta = [c.cwd, c.branch, c.model].filter(Boolean).join(" · ");

  return (
    <article ref={root} className="conversation" aria-label={c.title || "Untitled conversation"}>
      <header className="conversation__head">
        <div className="conversation__titles">
          <h2 className="conversation__title" dir="auto">
            {c.title || "Untitled conversation"}
          </h2>
          <p className="conversation__meta" dir="auto">
            {meta}
          </p>
        </div>
        {liveTab !== null ? (
          <button type="button" className="conversation__action" onClick={() => onGoTo(liveTab)}>
            Go to session
          </button>
        ) : (
          <button type="button" className="conversation__action" onClick={onResume}>
            Resume
          </button>
        )}
      </header>
      {c.limited && (
        <p className="conversation__notice" role="note">
          {limitedNotice(c.version)}
        </p>
      )}
      {c.turns.map((t) => (
        <section
          key={t.index}
          className="turn"
          data-turn={t.index}
          data-target={t.index === scrollToTurn ? "true" : undefined}
          aria-labelledby={`${uid}-turn-${t.index}`}
        >
          <h3 className="turn__prompt" id={`${uid}-turn-${t.index}`} dir="auto">
            {t.prompt}
          </h3>
          {t.at_ms > 0 && (
            <time className="turn__time" dateTime={new Date(t.at_ms).toISOString()}>
              {when(t.at_ms)}
            </time>
          )}
          {t.items.length > 0 && (
            <div className="turn__items">
              {t.items.map((item, i) => (
                <Item key={i} item={item} cwd={c.cwd} onLink={onLink} />
              ))}
            </div>
          )}
        </section>
      ))}
    </article>
  );
}
