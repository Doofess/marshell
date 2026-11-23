import { openUrl } from "@tauri-apps/plugin-opener";
import { type ReactNode, type Ref, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ContextMenu } from "../../components/ContextMenu/ContextMenu";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { ChangesReply } from "../../generated/ChangesReply";
import { NO_FOLDER_LINE } from "../files/FileTree";
import { absoluteOf } from "../files/treeModel";
import { ChangesIcon, EmptyPanel } from "../pane/EmptyPanel";
import { paneFocusTarget } from "../pane/focusPane";
import { type ItemRange, DiffList, type ListResult, reach } from "./DiffList";
import { remembered, safeCommentHtml } from "../preview/markdown";
import { type ChangesChoice, type CommentItem, addressOf, changesChoice, changesMenuItems, headerMenuItems, lineMenuItems, openLineOf, openOf, settingLabel, threadMenuItems } from "./changesMenu";
import { type BoxAction, CommentBox, boxLabel } from "./CommentBox";
import { type BoxState, type ReplyState, type ThreadBox, afterReply, anchorKey, boxAfter, beginPost, canPost, closedThreadBox, openThreadBox, threadBoxAfter } from "./boxModel";
import { listen, replyHolder, setHeld } from "./replyStore";
import { type ReplyAction, ReplyBox, cleanReply } from "./ReplyBox";
import { MAX_COMMENTS, type Placement, anchorOf, place, renumbered } from "./comments";
import { type OpenBox, boxAnchor, commentsOf, elsewhere, givesWay, keptBox, sendMessageOf, shownComments, unkept, withFolds } from "./drafts";
import { copy } from "./copy";
import type { AskRequest } from "../../generated/AskRequest";
import type { CiCheck } from "../../generated/CiCheck";
import type { MergeMethod } from "../../generated/MergeMethod";
import type { PullActionKind } from "../../generated/PullActionKind";
import type { PullStatus } from "../../generated/PullStatus";
import type { PullThreads } from "../../generated/PullThreads";
import type { ThreadActionKind } from "../../generated/ThreadActionKind";
import type { CheckAction } from "./ChecksList";
import { ask, failureOf, pullAction, threadAction } from "./githubApi";
import { githubCopy as gc } from "./githubCopy";
import { CreatePullSheet } from "./CreatePullSheet";
import { MergeDialog } from "./MergeDialog";
import { PullBar } from "./PullBar";
import { PushSheet } from "./PushSheet";
import { type BarButton, type BarCommand, type BarMenuItem, announcementsOf, barOf, commandOf } from "./pullModel";
import { threadCopy } from "./threadCopy";
import { threadsBarOf, newCommentsOf, withThreads } from "./threadsBar";
import { addAllOf, commentSay, discarded, githubItemsOf, hasList, inDiffOrder, nextToResolve, placementsOf, readNotice, sendCount, sendNotice } from "./threadsPanel";
import { afterSendPicked, batchMessageOf, githubMessageOf, pickedItems, pruned, sendBudget, withPicked, withoutPicked } from "./threadsSend";
import { authorOf, toResolve } from "./threads";
import { recountKey, useGithub } from "./useGithub";
import { threadsTarget, useThreads } from "./useThreads";
import { type Item, type Row, type Section, cutFiles, focusIndex, itemsOf, sectionsOf } from "./model";
import { reviewLabel } from "./review";
import { SendBar } from "./SendBar";
import { type SendResult, afterSend, askNotice } from "./send";
import { useChanges } from "./useChanges";
import { useDiffColours } from "./useDiffColours";
import { useChangesSettings } from "./changesSettings";
import "./ChangesPanel.css";

export type ChangesPanelProps = {
  session: string;
  agent: string | null;
  agentName: string;
  shown: boolean;
  drafts: ChangesDrafts | undefined;
  onDrafts(next: ChangesDrafts | null): void;
  onOpen(o: { path: string; line?: number; keep: boolean }): void;
  onEscape(): void;
  /** Hands the comments' message to the session; the panel clears the comments only when `sent` is true. `left` counts the picked GitHub comments that didn't fit: the notice says so (`sendNotice`). */
  onSend(text: string, count: number, left?: number): Promise<SendResult>;
  onReview(base: string | null): void;
  /** "clipboard" for a session with no prompt line: the Send button reads Copy. */
  via: "paste" | "clipboard";
  ref?: Ref<ChangesHandle>;
  announce(text: string): void;
  /** The strip's title: "4 files · +120 −18", or "Changes" while there is nothing to count. */
  onSummary(text: string): void;
  /** The strip's ⋯ menu while it is open: where it opened and the button to give the focus back to. */
  menu: { anchor: { x: number; y: number }; opener: HTMLElement } | null;
  onMenuClose(): void;
  /** The session's own name, for what a typed request says; the agent's name when absent. */
  sessionName?: string;
  /** A request was typed into the session's prompt: the keyboard goes there. */
  onTyped?(): void;
};

export { listKey } from "./DiffList";

/** What the palette's Changes commands call on the mounted panel. Each announces why when it can't act. */
export type ChangesHandle = { refresh(): void; review(): void; send(): void; createPull(): void; push(): void; merge(): void; openPull(): void; nextComment(): void; addComments(): void };

/** A GitHub comment's body as safe HTML, remembered so the list's memo holds. */
const commentHtml = remembered(safeCommentHtml, 500);

/** The strip's lines and, under them, the pull request bar: drawn over a diff and over an empty one. */
export function ChangesHead({ strip, bar }: { strip: ReactNode; bar: ReactNode }) {
  return (
    <div className="changes__top">
      <div className="changes__strip">{strip}</div>
      {bar}
    </div>
  );
}


/** The panel's empty states, each with its mark; only a failed read has an action. */
export function ChangesEmpty({ reply, noFolder, onRetry }: { reply: ChangesReply | null; noFolder: boolean; onRetry(): void }) {
  // Nothing read yet: the panel stays blank until the first reply, and `focusPane` waits on the mark.
  if (reply === null && !noFolder) return <div data-changes-blank="" />;
  if (noFolder || reply === null) return <EmptyPanel icon={ChangesIcon} title={copy.emptyFailed("").title} line={NO_FOLDER_LINE} actions={[]} />;
  switch (reply.kind) {
    case "no_repo": {
      const c = copy.emptyRepo(reply.folder);
      return <EmptyPanel icon={ChangesIcon} title={c.title} line={c.line} actions={[]} />;
    }
    case "failed": {
      const c = copy.emptyFailed(reply.reason ?? "");
      return <EmptyPanel icon={ChangesIcon} title={c.title} line={c.line} actions={[{ label: c.action, icon: null, onClick: onRetry }]} />;
    }
    case "refused": {
      const c = copy.emptyFailed(reply.reason ?? "");
      return <EmptyPanel icon={ChangesIcon} title={c.title} line={c.line} actions={[]} />;
    }
    default: {
      const c = copy.emptyNone(reply.base);
      return <EmptyPanel icon={ChangesIcon} title={c.title} line={c.line} actions={[]} />;
    }
  }
}

/** The lines under the strip's line: no base or unrelated histories, the new files past 200 or a look for them that was cut, and the files a cut diff leaves out. */
export function noticesOf(reply: ChangesReply): string[] {
  const cut = cutFiles(reply);
  return [
    reply.base === null ? copy.noBase : reply.unrelated ? copy.unrelated(reply.base) : null,
    reply.more_new > 0 ? copy.moreNew(reply.more_new) : null,
    reply.new_cut ? copy.newCut : null,
    reply.pr_base_unfetched !== null ? copy.prBaseUnfetched(reply.pr_base_unfetched) : null,
    cut > 0 ? copy.cut(cut) : null,
  ].filter((t): t is string => t !== null);
}

/**
 * The strip's lines over an empty diff: only while comments on another checkout wait, so they can still be sent or discarded (the Send bar
 * comes with them). Null for a bare empty state.
 */
export function emptyNotices(listed: ChangesReply | null, drafts: ChangesDrafts | undefined): string[] | null {
  const away = listed === null ? null : elsewhere(drafts, listed.folder);
  return listed === null || away === null ? null : [...noticesOf(listed), away];
}

/** Where the keyboard is, kept as keys so a poll that moves rows doesn't move it. */
type Keys = { focus: string | null; range: { from: string; to: string } | null };

export function ChangesPanel(p: ChangesPanelProps) {
  const { settings, failed, set: setSetting } = useChangesSettings();
  const hide = settings?.hide_whitespace ?? false;
  const wrap = settings?.wrap ?? false;
  const { reply, noFolder, refresh } = useChanges({ session: p.session, shown: p.shown, hideWhitespace: hide });
  const listed = reply?.kind === "listed" ? reply : null;
  const { drafts } = p;

  // The pull request bar: what GitHub says about this checkout, and what the bar's buttons do.
  const github = useGithub(p.session, p.shown, recountKey(listed), settings?.pull_requests ?? false);
  const gh = github.reply;
  const ghAgent = p.agent === null ? null : p.agentName;
  const now = Date.now();
  // GitHub's review comments of the bar's pull request: read after each ask of GitHub and after a refresh.
  const { threads, failed: readFailed, refresh: refreshThreads } = useThreads(p.session, gh, github.asked);
  const threadsRef = useRef(threads);
  threadsRef.current = threads;
  const plainBar = barOf(gh, ghAgent, now);
  const barView = plainBar === null ? null : withThreads(plainBar, threadsBarOf(threads, drafts));
  const [talkOpen, setTalkOpen] = useState(false);
  useEffect(() => setTalkOpen(false), [p.session]);
  const showOld = settings?.resolved_comments ?? false;
  const [note, setNote] = useState<string | null>(null);
  const before = useRef<typeof gh>(null);
  useEffect(() => {
    if (gh !== null) for (const t of announcementsOf(before.current, gh)) p.announce(t);
    before.current = gh;
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per new reply
  }, [gh]);
  useEffect(() => setNote(null), [p.session]);

  // The comment box (its text lives here, so a poll never loses it), the Send bar's question and a send under way.
  const [open, setOpen] = useState<OpenBox | null>(null);
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  // The box on a GitHub thread (a reply or the user's note), the reply text kept per thread and the threads with a reply out. Held per session
  // outside the panel (`replyStore`), so the check that stops a second post and the mark that a reply is out happen in one tick and outlive the box,
  // a redraw and a remount of the panel (it is keyed by the session); `setReply` sets it, and the listener below draws.
  const replyRef = replyHolder(p.session);
  const [, drawReply] = useState(0);
  useEffect(() => listen(p.session, () => drawReply((n) => n + 1)), [p.session]);
  const setReply = (next: ReplyState) => setHeld(p.session, next);
  const threadBox = replyRef.current.box;

  // Another checkout's comments aren't listed against this one's files (the strip says where they are).
  const comments = useMemo(() => (listed ? shownComments(drafts, listed.folder) : []), [listed, drafts]);
  // The open box's file stays, under "gone", when it leaves the diff, so the box and its text stay too.
  const openPath = open?.state.anchor.path;
  const sections = useMemo(() => (listed ? sectionsOf(listed, drafts?.folds ?? {}, comments, hide, openPath) : []), [listed, drafts, comments, hide, openPath]);
  /** The files of the diff shown now: a fold is kept for these alone. */
  const inDiff = useMemo(() => new Set(listed?.files.map((f) => f.path)), [listed]);
  const placements = useMemo(() => new Map<string, Placement>(comments.map((c) => [c.id, place(c, sections.find((s) => s.path === c.path))])), [sections, comments]);
  const githubItems = useMemo(() => githubItemsOf(threads, drafts, { showOld, talkOpen, sections, agent: ghAgent }), [threads, drafts, showOld, talkOpen, sections, ghAgent]);
  const items = useMemo<Item[]>(() => itemsOf(sections, comments, placements, githubItems), [sections, comments, placements, githubItems]);
  const draftsRef = useRef(drafts);
  draftsRef.current = drafts;
  const count = sendCount(drafts, threads);
  // What the checkout and base are for a drafts' save with no comment yet (the diff's, else the bar's checkout).
  const shownNow = { folder: listed?.folder ?? gh?.checkout ?? "", base: listed?.base ?? null };
  // A thread's box closes with its thread (resolved elsewhere, another branch, a folded file); a reply's text stays kept for the thread.
  useEffect(() => {
    const cur = replyRef.current;
    if (cur.box !== null && threadBoxAfter(items, cur.box) < 0) setReply(closedThreadBox(cur));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only a new list can take the thread away
  }, [items]);
  useEffect(() => {
    const cur = replyRef.current;
    if (cur.box !== null) setReply(closedThreadBox(cur));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per session
  }, [p.session]);

  // A new answer: the picks and sent marks GitHub no longer holds go, and the comments the user hasn't seen are announced.
  const seenThreads = useRef<PullThreads | null>(null);
  useEffect(() => {
    if (threads === null) {
      seenThreads.current = null;
      return;
    }
    const said = newCommentsOf(seenThreads.current, threads);
    seenThreads.current = threads;
    if (said !== null) p.announce(said);
    const kept = pruned(draftsRef.current, threads);
    if (kept !== undefined && kept !== draftsRef.current) p.onDrafts(kept);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per new answer
  }, [threads]);
  // Counts the times the keyboard goes back to the item it was on once the box or the bar is gone.
  const [back, setBack] = useState(0);

  const colours = useDiffColours(sections);
  const box = useRef<HTMLDivElement>(null);
  // The menu of an item (by its key, so a poll that moves rows keeps it) and the element to give the focus back to.
  const [itemMenu, setItemMenu] = useState<{ key: string; anchor: { x: number; y: number }; opener: HTMLElement | null } | null>(null);

  const [keys, setKeys] = useState<Keys>({ focus: null, range: null });
  // A reply that changes the focused line (its key is gone) leaves the keyboard at the index it was on, not at the top.
  const lastFocus = useRef(0);
  const focus = focusIndex(items, keys.focus, lastFocus.current);
  lastFocus.current = focus;
  const range = useMemo<ItemRange | null>(() => {
    if (keys.range === null) return null;
    const from = items.findIndex((i) => i.key === keys.range!.from);
    const to = items.findIndex((i) => i.key === keys.range!.to);
    return from < 0 || to < 0 ? null : { from, to };
  }, [items, keys.range]);

  const summary = useMemo(() => {
    if (!listed) return "Changes";
    return copy.summary(
      sections.filter((s) => s.note !== "gone" && s.note !== "cut").length,
      sections.reduce((n, s) => n + s.added, 0),
      sections.reduce((n, s) => n + s.removed, 0),
    );
  }, [listed, sections]);
  const { onSummary } = p;
  useEffect(() => onSummary(summary), [summary, onSummary]);

  const fold = (path: string, folded: boolean) => {
    if (!listed) return;
    const no = unkept(path);
    if (no !== null) return p.announce(no);
    p.onDrafts(withFolds(drafts, listed, { [path]: folded }, inDiff));
  };
  const setRange = (next: ItemRange | null) => setKeys((k) => ({ ...k, range: next === null ? null : { from: items[next.from]!.key, to: items[next.to]!.key } }));
  const rowsIn = (r: ItemRange) => items.slice(Math.min(r.from, r.to), Math.max(r.from, r.to) + 1).filter((i) => i.kind === "row").length;

  // The keyboard goes to the item with `key` once the list draws it (a file unfolded for it takes a draw): the bar's buttons use this.
  const [goTo, setGoTo] = useState<{ key: string; at: number } | null>(null);
  const focusKey = (key: string) => {
    setKeys({ focus: key, range: null });
    setGoTo({ key, at: Date.now() });
  };
  useEffect(() => {
    if (goTo === null) return;
    if (Date.now() - goTo.at > 3000) return setGoTo(null);
    const at = items.findIndex((i) => i.key === goTo.key);
    if (at < 0) return;
    box.current?.querySelector<HTMLElement>(`[data-changes-index="${at}"]`)?.focus();
    setGoTo(null);
  }, [goTo, items]);
  /** The bar's "comments to resolve" button and the palette's command: the keyboard goes to the next thread to resolve, unfolding its file first. False when there is none. */
  const goToNextComment = (): boolean => {
    const next = nextToResolve(items, focus, toResolve(threads?.threads ?? []));
    if (next === null) return false;
    const key = `thread:${next.id}`;
    const file = items.some((i) => i.key === key) ? undefined : sections.find((s) => s.path === next.path);
    if (file?.folded) fold(file.path, false);
    focusKey(key);
    return true;
  };
  const toggleTalk = () => {
    const open = !talkOpen;
    setTalkOpen(open);
    const first = threads?.notes[0];
    if (open && first !== undefined) focusKey(`talk:${first.id}`);
  };

  useEffect(() => {
    if (count === 0) setAsking(false);
  }, [count]);

  // The keyboard goes back to its item once the box or the bar is gone (never because a poll changed the list), else to what the panel holds
  // (an empty state's button or title once a discard leaves no diff and no comment).
  useEffect(() => {
    if (back > 0) (box.current?.querySelector<HTMLElement>(`[data-changes-index="${focus}"]`) ?? paneFocusTarget(document, "changes"))?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only a request to go back moves the focus
  }, [back]);

  /** The items a comment or a line menu acts on: the range when the item is in it, else the item. */
  const heldItems = (item: Item): Item[] => {
    const at = items.indexOf(item);
    const lo = range === null ? -1 : Math.min(range.from, range.to);
    const hi = range === null ? -1 : Math.max(range.from, range.to);
    return at >= lo && at <= hi ? items.slice(lo, hi + 1) : [item];
  };

  // Set when the user opens the box, so it takes the focus once (a redraw that mounts it again doesn't).
  const focusBox = useRef(false);
  /** Where the box's rows are now. */
  const placeBox = (state: BoxState): Placement =>
    place({ ...state.anchor, id: "", text: "" }, sections.find((s) => s.path === state.anchor.path));

  /** The drafts with a box's text kept: at the lines its rows are on now, unless the shell moved to another checkout while it was written. */
  const keptNow = (current: OpenBox, folder: string): ChangesDrafts => {
    const here = current.on.folder === folder;
    const anchor = boxAnchor(current, folder, placeBox(current.state));
    return keptBox(drafts, current, anchor, crypto.randomUUID(), (c) => (here ? renumbered(c, placements.get(c.id)) : c));
  };

  const startBox = (item: Item | undefined) => {
    if (!listed || item === undefined) return;
    let next: { state: BoxState; text: string };
    if (item.kind === "comment") {
      const { id, text, ...anchor } = item.comment;
      next = { state: { anchor, id }, text };
    } else if (item.kind === "row") {
      const held = heldItems(item).filter((i): i is Extract<Item, { kind: "row" }> => i.kind === "row");
      const rows = held.map((i) => i.file.rows[i.at]!);
      if (held.length === 0 || rows.some((r) => r.kind === "hunk")) return;
      const ats = held.map((i) => i.at);
      next = { state: { anchor: anchorOf(item.file, Math.min(...ats), Math.max(...ats)), id: null }, text: "" };
    } else return;
    const way = givesWay(open, next.state);
    // The box asked for is the open one: it keeps its text and takes the keyboard.
    if (way === "same") return void box.current?.querySelector<HTMLTextAreaElement>(".changes__box-text")?.focus();
    // An open box with typed text gives way as Esc would have it: the text is kept as a draft, and its box closes whatever comes next.
    const kept = way === "keep" && open !== null ? keptNow(open, listed.folder) : null;
    if (kept !== null) {
      setOpen(null);
      p.onDrafts(kept);
      p.announce("Comment kept as a draft.");
    }
    const now = kept ?? drafts;
    if (next.state.id === null) {
      const away = elsewhere(now, listed.folder) ?? unkept(next.state.anchor.path);
      if (away !== null) return p.announce(away);
      if ((now?.comments.length ?? 0) >= MAX_COMMENTS) return p.announce(`A session holds at most ${MAX_COMMENTS} comments.`);
    }
    // A box on the same lines as the one that gave way is the same element, which mounts nothing: it is focused here instead.
    if (open !== null && anchorKey(open.state.anchor) === anchorKey(next.state.anchor)) box.current?.querySelector<HTMLTextAreaElement>(".changes__box-text")?.focus();
    else focusBox.current = true;
    // One box at a time: a thread's box gives way as its Esc would (a note is saved, a reply's text is kept).
    const mine = replyRef.current;
    if (mine.box !== null) {
      if (mine.box.kind === "note" && !saveNote(mine.box)) return;
      setReply(closedThreadBox(mine));
    }
    setOpen({ ...next, on: { folder: listed.folder, base: listed.base } });
  };

  /** The thread's or the note's item the box is under, by the thread's id. */
  const itemKeyOf = (thread: string) => `thread:${thread}`;
  /**
   * The note box's text as the thread's note: a note adds the thread to the send (as a pick) and says so; an empty one on a picked thread
   * clears the note and leaves it picked; an empty one on any other does nothing. False when the send is full, so the box stays.
   */
  const saveNote = (b: ThreadBox): boolean => {
    const target = threadsTarget(gh);
    if (target === null) return true;
    const d = draftsRef.current;
    const text = b.text.trim();
    if (text === "" && (d?.picked.find((x) => x.id === b.thread)?.note ?? "") === "") return true;
    const next = withPicked(d, shownNow, target.number, b.thread, text);
    if (next === null) {
      p.announce(threadCopy.tooMany);
      return false;
    }
    p.onDrafts(next);
    if (text !== "") p.announce(threadCopy.noteAdded(sendCount(next, threads)));
    return true;
  };
  /** Opens the reply box or the note box under a thread (the keyboard goes to it); the box that is open gives way as its Esc would. */
  const startThreadBox = (kind: ThreadBox["kind"], item: CommentItem) => {
    if (item.kind !== "thread") return;
    const thread = item.thread.id;
    const cur = replyRef.current;
    if (cur.box?.kind === kind && cur.box.thread === thread) return void box.current?.querySelector<HTMLTextAreaElement>(".changes__box-text")?.focus();
    if (kind === "reply" && !item.thread.can_reply) return p.announce(threadCopy.cantReply);
    if (cur.box?.kind === "note" && !saveNote(cur.box)) return;
    // A draft comment box with typed text is kept as a draft first, as `startBox` keeps it.
    if (open !== null) {
      if (listed && open.text.trim() !== "") {
        p.onDrafts(keptNow(open, listed.folder));
        p.announce("Comment kept as a draft.");
      }
      setOpen(null);
    }
    focusBox.current = true;
    setReply(openThreadBox(cur, kind, thread, item.state.note));
  };
  /** The thread box's Esc, Enter or close: a reply closes (never while it is out) and keeps its text; a note is saved. The keyboard goes back to the thread's item. */
  const closeThreadBox = () => {
    const cur = replyRef.current;
    const b = cur.box;
    if (b === null) return;
    if (b.kind === "reply" && !canPost(cur, b.thread)) return;
    if (b.kind === "note" && !saveNote(b)) return;
    setReply(closedThreadBox(cur));
    focusKey(itemKeyOf(b.thread));
  };
  /** Posts the open reply. The check that no reply to the thread is out and the mark that one is happen in the same tick, before the request starts. */
  const postReply = async () => {
    const target = threadsTarget(gh);
    const first = replyRef.current;
    const b = first.box;
    if (target === null || b === null || b.kind !== "reply") return;
    const body = cleanReply(b.text);
    if (body.trim() === "") return;
    const started = beginPost(first, b.thread);
    if (started === null) return;
    // The session the request is for: its answer lands in this session's holder, whether or not this panel is still mounted.
    const session = p.session;
    setHeld(session, started);
    const thread = b.thread;
    let result: Parameters<typeof afterReply>[2];
    try {
      await threadAction(session, { ...target, thread, action: "reply", body });
      result = { ok: true };
    } catch (e) {
      result = { ok: false, reason: failureOf(e).reason };
    }
    const now = replyHolder(session).current;
    const there = now.box?.kind === "reply" && now.box.thread === thread;
    setHeld(session, afterReply(now, thread, result));
    if (result.ok) {
      p.announce(threadCopy.replied(target.number));
      if (there) focusKey(itemKeyOf(thread));
    } else p.announce(result.reason);
    github.refresh();
    refreshThreads();
  };
  const onReplyAction = (a: ReplyAction) => {
    if (a === "post") void postReply();
    else closeThreadBox();
  };

  const closeBox = (action: BoxAction) => {
    const current = open;
    setOpen(null);
    setBack((n) => n + 1);
    if (!current || !listed) return;
    if (action === "close" || current.text.trim() === "") return;
    const next = keptNow(current, listed.folder);
    p.onDrafts(next);
    p.announce(action === "add" ? `Comment added. ${next.comments.length} to send.` : "Comment kept as a draft.");
  };

  const removeComment = (index: number) => {
    const item = items[index];
    if (!item || item.kind !== "comment" || !drafts) return;
    const before = items[index - 1];
    if (before) setKeys({ focus: before.key, range: null });
    p.onDrafts({ ...drafts, comments: drafts.comments.filter((c) => c.id !== item.comment.id) });
    p.announce("Comment deleted.");
    setBack((n) => n + 1);
  };

  const send = async () => {
    const d = draftsRef.current;
    const th = threadsRef.current;
    const own = d?.comments ?? [];
    const picked = pickedItems(d, th);
    if (own.length + picked.length === 0 || (own.length > 0 && !listed)) return p.announce("There are no comments to send.");
    if (busy) return;
    // The user's own comments first, then what fits of GitHub's, in the diff's order (only what the reply lists and the user sees).
    const draftsPart = d !== undefined && own.length > 0 && listed ? sendMessageOf(d, listed, sections, placements) : null;
    const number = th?.number ?? 0;
    const pickedPlaces = placementsOf(picked.flatMap((i) => (i.kind === "thread" ? [i.thread] : [])), sections);
    const onGithub = githubMessageOf(inDiffOrder(picked, sections, pickedPlaces), number, pickedPlaces, sendBudget(draftsPart));
    if (draftsPart === null && onGithub.text === null) return p.announce(threadCopy.leftOver(onGithub.left));
    setBusy(true);
    let result: SendResult;
    try {
      result = await p.onSend(batchMessageOf(draftsPart, onGithub.text), own.length + onGithub.went.length, onGithub.left);
    } finally {
      setBusy(false);
    }
    const latest = draftsRef.current;
    if (result.sent && latest) p.onDrafts(afterSendPicked(afterSend(latest, result, own), number, onGithub.went));
  };

  // The pull request bar's actions. The ones that need a sheet are Task 14's: each is the one place it fills.
  // At most one sheet is open; the panel holds which.
  // What a sheet needs from the reply is taken when it opens: a reply that changes under it never takes it down mid-push.
  type Sheet = { kind: "create"; repo: string } | { kind: "push" } | { kind: "output"; id: string } | { kind: "merge"; checkout: string; pull: PullStatus; method: MergeMethod; auto: boolean };
  const [sheet, setSheet] = useState<Sheet | null>(null);
  useEffect(() => setSheet(null), [p.session]);
  // A merge takes away the button the keyboard came from, at once or with the next reply. Until that reply is drawn, a
  // keyboard left on nothing goes to the bar.
  const focusBarAfterMerge = useRef<{ from: typeof gh } | null>(null);
  useEffect(() => {
    const f = focusBarAfterMerge.current;
    if (f === null || sheet !== null) return;
    const active = document.activeElement;
    if (active === null || active === document.body) box.current?.querySelector<HTMLElement>(".pull-bar button")?.focus();
    if (gh !== f.from) focusBarAfterMerge.current = null;
  }, [sheet, gh]);
  const createPull = () => gh && setSheet({ kind: "create", repo: gh.repo });
  const push = () => setSheet({ kind: "push" });
  const merge = (method?: MergeMethod, auto = false) => {
    const m = method ?? gh?.merge_methods[0];
    if (m !== undefined && gh?.pull) setSheet({ kind: "merge", checkout: gh.checkout, pull: gh.pull, method: m, auto });
  };
  const showOutput = () => {
    if (gh?.push) setSheet({ kind: "output", id: gh.push });
  };
  const fail = (text: string) => {
    setNote(text);
    p.announce(text);
  };
  const openHttps = (url: string | null | undefined) => {
    if (typeof url !== "string" || !url.startsWith("https://")) return p.announce(gc.noAddress);
    void openUrl(url).catch(() => fail(gc.noAddress));
  };
  const copyOut = (text: string, said: string) => void navigator.clipboard.writeText(text).then(() => p.announce(said), () => p.announce(gc.couldntCopy));
  const pullDo = async (action: PullActionKind) => {
    const pull = gh?.pull;
    if (!gh || !pull) return;
    setNote(null);
    try {
      await pullAction(p.session, { checkout: gh.checkout, id: pull.id, number: pull.number, action });
    } catch (e) {
      fail(failureOf(e).reason);
    }
    github.refresh();
  };
  const askAgent = async (req: AskRequest) => {
    setNote(null);
    const name = p.sessionName ?? p.agentName;
    try {
      const r = await ask(p.session, req);
      if (r.form === "copy") copyOut(r.text, req.kind === "switch_and_pull" ? gc.copiedCommands : gc.copiedRequest(name));
      else {
        p.announce(gc.typedRequest(name));
        p.onTyped?.();
      }
    } catch (e) {
      fail(askNotice(e, name));
    }
  };
  const onBarMain = (button: BarButton, method?: MergeMethod) => {
    switch (button.id) {
      case "push":
        return push();
      case "create":
        return createPull();
      case "merge":
      case "auto_merge":
        return merge(method, button.id === "auto_merge");
      case "show_output":
        return showOutput();
      case "ready":
        return void pullDo("ready");
      case "get_gh":
        return openHttps(button.url);
      case "copy_sign_in":
        return copyOut(button.text ?? "", button.done ?? gc.copied);
      case "retry":
        return github.refresh();
    }
  };
  const onBarMenu = (item: BarMenuItem) => {
    switch (item.id) {
      case "push":
        return push();
      case "merge":
        return merge();
      case "draft":
        return void pullDo("draft");
      case "cancel_auto_merge":
        return void pullDo("cancel_auto_merge");
      case "hide":
        return void setSetting({ pull_requests: false }).then(() => p.announce(gc.hidden), () => p.announce(gc.hideFailed));
      case "ask_open":
        return void askAgent({ kind: "open_pull", copy: false });
      case "ask_switch":
      case "copy_switch":
        return void (gh?.pull && askAgent({ kind: "switch_and_pull", copy: item.id === "copy_switch", number: gh.pull.number }));
      case "add_unresolved":
        return addAll();
      case "open_on_github":
        return openHttps(item.url);
      case "copy_link":
        return copyOut(item.url ?? "", gc.copiedLink);
    }
  };
  /** "Add all unresolved": every unresolved comment of the pull request goes into the send. */
  const addAll = () => {
    if (threads === null) return p.announce(threadCopy.nothingToAdd);
    const out = addAllOf(draftsRef.current, shownNow, threads);
    if (out.drafts !== null) p.onDrafts(out.drafts);
    p.announce(out.say);
  };
  /** Adds a thread or a note to the send, or takes it out. */
  const pick = (id: string, on: boolean) => {
    const target = threadsTarget(gh);
    const d = draftsRef.current;
    if (target === null) return;
    if (!on) {
      if (d !== undefined) p.onDrafts(withoutPicked(d, id));
      return p.announce(threadCopy.unpicked);
    }
    const next = withPicked(d, shownNow, target.number, id);
    if (next === null) return p.announce(threadCopy.tooMany);
    p.onDrafts(next);
    p.announce(threadCopy.picked(sendCount(next, threads)));
  };
  /** Resolves or unresolves a thread of the bar's pull request; either way the bar and the comments are read again. */
  const threadDo = async (thread: string, action: Extract<ThreadActionKind, "resolve" | "unresolve">) => {
    const target = threadsTarget(gh);
    if (target === null) return;
    setNote(null);
    try {
      await threadAction(p.session, { ...target, thread, action, body: null });
      p.announce(action === "resolve" ? threadCopy.resolvedSaid : threadCopy.unresolvedSaid);
    } catch (e) {
      fail(failureOf(e).reason);
    }
    github.refresh();
    refreshThreads();
  };
  const onBarCheck = (action: CheckAction, check: CiCheck) => {
    if (action === "open") return openHttps(check.url);
    if (action === "copy_link") return check.url === null ? p.announce(gc.noCheckAddress) : copyOut(check.url, gc.copiedLink);
    if (gh?.pull) void askAgent({ kind: "fix_check", copy: p.agent === null, number: gh.pull.number, name: check.name, workflow: check.workflow, url: check.url, run_id: check.run_id });
  };
  // The palette's commands: the bar's own action, or why it can't run now.
  const barCommand = (command: BarCommand) => {
    const what = commandOf(barView, command);
    if ("main" in what) return onBarMain(what.main);
    if ("menu" in what) return onBarMenu(what.menu);
    p.announce(barView === null && settings?.pull_requests !== false ? gc.unavailable : what.say);
  };
  const refreshAll = () => {
    refresh();
    github.refresh();
  };
  const closeSheet = () => setSheet(null);
  const sheetDone = () => {
    if (sheet?.kind === "merge") focusBarAfterMerge.current = { from: gh };
    setSheet(null);
    github.refresh();
  };
  const sheetNode =
    sheet === null ? null : sheet.kind === "create" ? (
      <CreatePullSheet key="create" tab={p.session} repo={sheet.repo} announce={p.announce} onClose={closeSheet} onDone={sheetDone} />
    ) : sheet.kind === "push" ? (
      <PushSheet key="push" tab={p.session} announce={p.announce} onClose={closeSheet} onDone={sheetDone} />
    ) : sheet.kind === "output" ? (
      <PushSheet key="output" tab={p.session} resume={sheet.id} announce={p.announce} onClose={closeSheet} onDone={sheetDone} />
    ) : (
      <MergeDialog key="merge" tab={p.session} checkout={sheet.checkout} pull={sheet.pull} method={sheet.method} auto={sheet.auto} announce={p.announce} onClose={closeSheet} onDone={sheetDone} />
    );
  // Over the whole window, where every other dialog is drawn: the side pane is too narrow for a sheet, and a page beside it stays covered.
  const sheetHost = sheetNode === null ? null : (box.current?.closest(".live-app") ?? null);
  const sheetAt = sheetNode !== null && sheetHost !== null ? createPortal(sheetNode, sheetHost) : sheetNode;
  const nextComment = () => {
    if (!goToNextComment()) p.announce(threadCopy.nothingToResolve);
  };
  /** The palette's commands about the comments: the action, or why this bar has none. */
  const commentCommand = (action: () => void) => {
    const say = commentSay(barView, settings?.pull_requests === false);
    if (say === null) action();
    else p.announce(say);
  };
  const bar =
    barView === null ? null : (
      <PullBar view={barView} agent={ghAgent} now={now} onMain={onBarMain} onMenu={onBarMenu} onCheck={onBarCheck} onEscape={p.onEscape} onResolve={nextComment} onTalk={toggleTalk} talkOpen={talkOpen} />
    );

  useImperativeHandle(p.ref, () => ({
    refresh: refreshAll,
    review: () => (listed ? p.onReview(listed.base) : p.announce("There are no changes to review.")),
    send: () => void send(),
    createPull: () => barCommand("create"),
    push: () => barCommand("push"),
    merge: () => barCommand("merge"),
    openPull: () => barCommand("open"),
    nextComment: () => commentCommand(nextComment),
    addComments: () => commentCommand(addAll),
  }));

  const onKey = (r: ListResult) => {
    const target = items[r.focus];
    if (target !== undefined) setKeys((k) => ({ focus: target.key, range: r.range === undefined ? k.range : null }));
    if (r.range) {
      setRange(r.range);
      p.announce(copy.selected(rowsIn(r.range)));
    }
    if (r.fold !== undefined) fold(r.fold, true);
    if (r.unfold !== undefined) fold(r.unfold, false);
    if (r.open === "comment") startBox(target);
    if (r.remove) removeComment(r.focus);
    if (r.open === "menu") {
      const el = box.current?.querySelector<HTMLElement>(`[data-changes-index="${r.focus}"]`) ?? null;
      const at = el?.getBoundingClientRect();
      openItemMenu(target, at ? { x: at.left + 24, y: at.bottom } : { x: 0, y: 0 }, el);
    }
    if (r.escape) p.onEscape();
  };

  const onClickItem = (index: number, mods: { shift: boolean }) => {
    const item = items[index];
    if (item === undefined) return;
    if (mods.shift && item.kind === "row") {
      const anchor = range?.from ?? focus;
      const to = reach(items, anchor, index);
      const next = to === anchor ? null : { from: anchor, to };
      setKeys({ focus: items[to]!.key, range: next === null ? null : { from: items[anchor]!.key, to: items[to]!.key } });
      if (next) p.announce(copy.selected(rowsIn(next)));
      return;
    }
    setKeys({ focus: item.key, range: null });
    if (item.kind === "file" && item.file.rows.length > 0) fold(item.file.path, !item.file.folded);
    else if (item.kind === "show") fold(item.file.path, false);
  };

  const absolute = (file: Section) => (listed ? absoluteOf(listed.folder, file.path, listed.folder.includes("\\")) : file.path);
  const openFile = (file: Section, line: number | undefined) => p.onOpen({ path: absolute(file), ...(line === undefined ? {} : { line }), keep: true });
  // A double-click opens what the menu's Open opens, and says why for a deleted file.
  const onOpenItem = (index: number) => {
    const to = listed ? openOf(items[index]) : null;
    if (to === null) return;
    if ("refused" in to) return p.announce(to.refused);
    openFile(to.file, to.line);
  };

  // Menus: a header and a row have one each; the strip's ⋯ menu comes from the app as `p.menu`.
  const openItemMenu = (item: Item | undefined, anchor: { x: number; y: number }, opener: HTMLElement | null) => {
    if (item?.kind === "file" || item?.kind === "row" || item?.kind === "thread" || item?.kind === "talk") setItemMenu({ key: item.key, anchor, opener });
  };
  const onMenuItem = (index: number, at: { x: number; y: number }) => {
    const item = items[index];
    if (item === undefined) return;
    setKeys({ focus: item.key, range: null });
    openItemMenu(item, at, box.current?.querySelector<HTMLElement>(`[data-changes-index="${index}"]`) ?? null);
  };
  const menuItem = itemMenu === null ? undefined : items.find((i) => i.key === itemMenu.key);
  /** The rows a line menu acts on: the range when the item is in it, else the item. A range leaves out hunk headers. */
  const menuRows = (item: Item): Row[] => {
    const held = heldItems(item);
    return held.flatMap((i) => (i.kind === "row" && (held.length === 1 || i.file.rows[i.at]!.kind !== "hunk") ? [i.file.rows[i.at]!] : []));
  };
  const closeItemMenu = (refocus: boolean) => {
    const m = itemMenu;
    setItemMenu(null);
    if (!m || !refocus) return;
    const index = items.findIndex((i) => i.key === m.key);
    (m.opener?.isConnected ? m.opener : box.current?.querySelector<HTMLElement>(`[data-changes-index="${index}"]`))?.focus();
  };
  const copyText = (text: string, said: string, failedText: string) => void navigator.clipboard.writeText(text).then(() => p.announce(said), () => p.announce(failedText));
  const runItemMenu = (id: string) => {
    const item = menuItem;
    closeItemMenu(true);
    if (item?.kind === "file") {
      if (id === "open") openFile(item.file, undefined);
      else if (id === "fold") fold(item.file.path, !item.file.folded);
      else if (id === "copy-path") copyText(absolute(item.file), "Copied the path.", "Couldn't copy the path.");
    } else if (item?.kind === "row") {
      if (id === "open") openFile(item.file, openLineOf(item.file.rows, item.at));
      else if (id === "copy") {
        const rows = menuRows(item);
        copyText(rows.map((r) => r.text).join("\n"), rows.length === 1 ? "Copied the line." : `Copied ${rows.length} lines.`, "Couldn't copy the lines.");
      }
      else if (id === "comment") startBox(item);
    } else if (item?.kind === "thread" || item?.kind === "talk") runCommentMenu(item, id);
  };
  /** A thread's or a note's menu row. */
  const runCommentMenu = (item: CommentItem, id: string) => {
    const at = item.kind === "thread" ? item.thread.id : item.note.id;
    if (id === "pick" || id === "unpick") pick(at, id === "pick");
    else if (id === "note" || id === "reply") startThreadBox(id, item);
    else if (id === "resolve" || id === "unresolve") {
      if (item.kind === "thread" && (id === "resolve" ? item.thread.can_resolve : item.thread.can_unresolve)) void threadDo(at, id);
    } else if (id === "open") openHttps(addressOf(item));
    else if (id === "copy-link") {
      const url = addressOf(item);
      if (url === null) p.announce(gc.noAddress);
      else copyOut(url, gc.copiedLink);
    }
  };
  const foldAll = (folded: boolean) => {
    if (!listed) return;
    const all = Object.fromEntries(sections.filter((s) => s.rows.length > 0).map((s) => [s.path, folded]));
    p.onDrafts(withFolds(drafts, listed, all, inDiff));
  };
  const runMenu = (id: string) => {
    const choice: ChangesChoice | null = changesChoice(id);
    const opener = p.menu?.opener;
    p.onMenuClose();
    opener?.focus();
    if (choice === null) return;
    if (choice.kind === "refresh") refreshAll();
    else if (choice.kind === "github") {
      if (choice.from === "main") {
        if (barView?.main?.id === choice.id) onBarMain(barView.main);
      } else {
        const item = barView?.menu.find((m) => m.id === choice.id);
        if (item) onBarMenu(item);
      }
    } else if (choice.kind === "review") p.onReview(listed?.base ?? null);
    else if (choice.kind === "fold-all") foldAll(true);
    else if (choice.kind === "unfold-all") foldAll(false);
    else if (choice.kind === "setting") {
      const { key } = choice;
      const on = !(settings?.[key] ?? false);
      void setSetting({ [key]: on }).then(
        () => p.announce(`${settingLabel(key)}: ${on ? "on" : "off"}`),
        () => p.announce("The setting wasn't saved."),
      );
    }
  };
  const menus = (
    <>
      {p.menu && (
        <ContextMenu
          label="Changes menu"
          items={changesMenuItems({ settings, failed, review: p.agent === null ? null : reviewLabel(p.agent, p.agentName), hasFiles: sections.some((s) => s.rows.length > 0), github: barView?.menu ?? [], main: barView?.main ?? null })}
          anchor={p.menu.anchor}
          onRun={runMenu}
          onClose={({ refocus }) => {
            const opener = p.menu?.opener;
            p.onMenuClose();
            if (refocus) opener?.focus();
          }}
        />
      )}
      {itemMenu && menuItem && (menuItem.kind === "file" || menuItem.kind === "row" || menuItem.kind === "thread" || menuItem.kind === "talk") && (
        <ContextMenu
          label={menuItem.kind === "file" ? "File menu" : menuItem.kind === "row" ? "Line menu" : threadCopy.threadMenu}
          items={menuItem.kind === "file" ? headerMenuItems(menuItem.file) : menuItem.kind === "row" ? lineMenuItems(menuItem, menuRows(menuItem).length) : threadMenuItems(menuItem, menuItem.state, menuItem.kind === "thread" && replyRef.current.out.has(menuItem.thread.id))}
          anchor={itemMenu.anchor}
          onRun={runItemMenu}
          onClose={({ refocus }) => closeItemMenu(refocus)}
        />
      )}
    </>
  );

  const sendBar = count > 0 && (
    <SendBar
      count={count}
      via={p.via}
      asking={asking}
      busy={busy}
      onSend={() => void send()}
      onAsk={() => setAsking(true)}
      onDiscard={() => {
        setAsking(false);
        if (drafts) p.onDrafts(discarded(drafts, threads?.number ?? null));
        p.announce(`Discarded ${commentsOf(count)}.`);
        setBack((n) => n + 1);
      }}
      onKeep={() => {
        setAsking(false);
        setBack((n) => n + 1);
      }}
    />
  );

  // The pull request's comments are read apart from the diff: while GitHub has a thread, an open note or a line to show, the list is drawn without files.
  const readLine = readNotice(gh?.pull?.number ?? null, readFailed);
  if (!listed || !hasList(listed, sections, githubItems)) {
    const empty = <ChangesEmpty reply={reply} noFolder={noFolder} onRetry={refresh} />;
    const around = emptyNotices(listed, drafts);
    if (around === null && bar === null && note === null && readLine === null) {
      return (
        <>
          {empty}
          {menus}
        </>
      );
    }
    return (
      <div className="changes" ref={box}>
        <ChangesHead
          strip={[...(around ?? []), ...(note === null ? [] : [note]), ...(readLine === null ? [] : [readLine])].map((t) => (
            <p key={t} className="changes__notice">
              {t}
            </p>
          ))}
          bar={bar}
        />
        {empty}
        {sendBar}
        {menus}
        {sheetAt}
      </div>
    );
  }

  const line = copy.line({ branch: listed.branch, detached: listed.detached, base: listed.base, ahead: listed.ahead, uncommitted: listed.uncommitted, moved: listed.moved, folder: listed.folder });
  const away = elsewhere(drafts, listed.folder);
  const notices = [...noticesOf(listed), ...(away === null ? [] : [away]), ...(note === null ? [] : [note]), ...(readLine === null ? [] : [readLine])];
  const boxAt = open === null ? -1 : boxAfter(items, sections, open.state);
  // The key stays the written anchor (the box keeps its text and caret); the label reads the lines it is on now.
  const boxLines = open === null ? null : boxAnchor(open, listed.folder, placeBox(open.state));
  const threadBoxAt = threadBox === null ? -1 : threadBoxAfter(items, threadBox);
  const threadItem = threadBox === null || threadBoxAt < 0 ? undefined : items[threadBoxAt];
  const threadAuthor = threadItem?.kind === "thread" ? (threadItem.thread.comments[0] === undefined ? threadCopy.from : authorOf(threadItem.thread.comments[0])) : threadCopy.from;
  const setThreadText = (text: string) => {
    const cur = replyRef.current;
    if (cur.box !== null) setReply({ ...cur, box: { ...cur.box, text } });
  };
  const draftNode =
    open !== null && boxLines !== null && boxAt >= 0
      ? {
          after: boxAt,
          node: <CommentBox key={anchorKey(open.state.anchor)} label={boxLabel(boxLines.start, boxLines.end)} value={open.text} onChange={(text) => setOpen((o) => (o === null ? o : { ...o, text }))} onAction={closeBox} focus={focusBox} />,
        }
      : undefined;
  // One box at a time: a thread's box and the draft comment box are never open together.
  const boxNode =
    threadBox === null || threadBoxAt < 0
      ? draftNode
      : {
          after: threadBoxAt,
          node:
            threadBox.kind === "reply" ? (
              <ReplyBox key={`reply:${threadBox.thread}`} label={threadCopy.replyLabel(threadAuthor)} value={threadBox.text} posting={replyRef.current.out.has(threadBox.thread)} error={threadBox.error} onChange={setThreadText} onAction={onReplyAction} focus={focusBox} />
            ) : (
              <CommentBox key={`note:${threadBox.thread}`} label={threadCopy.noteLabel(threadAuthor)} value={threadBox.text} onChange={setThreadText} onAction={closeThreadBox} focus={focusBox} />
            ),
        };
  return (
    <div className="changes" ref={box}>
      <ChangesHead
        strip={
          <>
            {line !== "" && <p className="changes__line">{line}</p>}
            {notices.map((t) => (
              <p key={t} className="changes__notice">
                {t}
              </p>
            ))}
          </>
        }
        bar={bar}
      />
      <DiffList
        items={items}
        focus={focus}
        range={range}
        wrap={wrap}
        onKey={onKey}
        onClickItem={onClickItem}
        onOpenItem={onOpenItem}
        onMenuItem={onMenuItem}
        colours={colours}
        box={boxNode}
        html={commentHtml}
        onLink={openHttps}
      />
      {sendBar}
      {menus}
      {sheetAt}
    </div>
  );
}
