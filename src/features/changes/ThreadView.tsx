import type { ReactNode } from "react";
import type { PullNote } from "../../generated/PullNote";
import type { ReviewThread } from "../../generated/ReviewThread";
import "../preview/Preview.css";
import { citation, type Placement } from "./comments";
import { threadCopy } from "./threadCopy";
import { authorOf, noteMarks, threadAnchor, threadMarks, type SendState } from "./threads";

/** A comment's body as HTML that is safe to insert (`safeCommentHtml`, remembered). It is the only thing of a comment drawn as markup. */
export type BodyHtml = (src: string) => string;

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };
/** A body as escaped text in a paragraph: what an item shows when the list is given no `html`. */
export const plainHtml: BodyHtml = (src) => `<p>${src.replace(/[&<>"]/g, (c) => ESCAPES[c]!)}</p>`;

/** Whether a link's address is an https one: the only kind a click hands on. An address that can't be read is not. */
export function httpsLink(href: string): boolean {
  try {
    return new URL(href).protocol === "https:";
  } catch {
    return false;
  }
}

const dayOf = (ms: number | null): string | null => (ms === null ? null : new Date(ms).toLocaleDateString("en-US", { month: "short", day: "numeric" }));

function Chips({ marks }: { marks: string[] }) {
  return marks.length === 0 ? null : (
    <span data-part="chips">
      {marks.map((m) => (
        <span key={m} className="changes__thread-chip">
          {m}
        </span>
      ))}
    </span>
  );
}

/** One comment: who (when it isn't the first) and when, then its body. */
function Comment({ author, day, ms, body, cut, html }: { author: string | null; day: string | null; ms: number | null; body: string; cut: boolean; html: BodyHtml }) {
  return (
    <div className="changes__thread-comment">
      {(author !== null || day !== null) && (
        <div data-part="meta">
          {author !== null && <span data-part="author">{author}</span>}
          {day !== null && ms !== null && (
            <time data-part="date" dateTime={new Date(ms).toISOString()}>
              {day}
            </time>
          )}
        </div>
      )}
      <div className="markdown changes__thread-body" dangerouslySetInnerHTML={{ __html: html(body) }} />
      {cut && <p className="changes__thread-note">{threadCopy.cut}</p>}
    </div>
  );
}

const Note = ({ children }: { children: ReactNode }) => <p className="changes__thread-note">{children}</p>;

/** Where a thread with no row of its own is: the line as the message cites it, or the file. */
function placeOf(t: ReviewThread, placed: Placement | null): string | null {
  if (t.file) return placed === null ? `${t.path} · ${threadCopy.fileComment}` : threadCopy.fileComment;
  if (placed !== null) return null;
  const anchor = threadAnchor(t);
  return anchor === null ? t.path : citation(anchor, true);
}

/** The inside of a thread's item. All of it is hidden from a screen reader: the item's label reads it. */
export function ThreadView(p: { thread: ReviewThread; placed: Placement | null; state: SendState; agent: string | null; html: BodyHtml }): ReactNode {
  const { thread: t, placed, state, agent, html } = p;
  const first = t.comments[0];
  const place = placeOf(t, placed);
  const more = t.more_comments;
  return (
    <div aria-hidden="true">
      <div className="changes__thread-head">
        <span data-part="from">{first === undefined ? threadCopy.from : `${threadCopy.from} · ${authorOf(first)}`}</span>
        <Chips marks={threadMarks(t, placed, state, agent)} />
      </div>
      {place !== null && <p data-part="place">{place}</p>}
      {t.comments.map((c, i) => (
        <Comment key={c.id} author={i === 0 ? null : authorOf(c)} day={dayOf(c.created_ms)} ms={c.created_ms} body={c.body} cut={c.cut} html={html} />
      ))}
      {more > 0 && <Note>{threadCopy.moreComments(more)}</Note>}
      {t.minimized > 0 && <Note>{threadCopy.minimized(t.minimized)}</Note>}
      {state.note !== "" && <Note>{threadCopy.yourNote(state.note)}</Note>}
    </div>
  );
}

/** The inside of a review's or a conversation comment's item. All of it is hidden from a screen reader, as a thread's is. */
export function NoteView(p: { note: PullNote; state: SendState; agent: string | null; html: BodyHtml }): ReactNode {
  const { note: n, state, agent, html } = p;
  const who = n.kind === "review" ? threadCopy.reviewBy(authorOf(n), n.verdict) : threadCopy.commentBy(authorOf(n));
  return (
    <div aria-hidden="true">
      <div className="changes__thread-head">
        <span data-part="from">{who}</span>
        <Chips marks={noteMarks(n, state, agent)} />
      </div>
      <Comment author={null} day={dayOf(n.created_ms)} ms={n.created_ms} body={n.body} cut={n.cut} html={html} />
      {state.note !== "" && <Note>{threadCopy.yourNote(state.note)}</Note>}
    </div>
  );
}
