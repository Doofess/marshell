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
import { type ChangesChoice, changesChoice, changesMenuItems, headerMenuItems, lineMenuItems, openLineOf, openOf, settingLabel } from "./changesMenu";
import { type BoxAction, CommentBox, boxLabel } from "./CommentBox";
import { type BoxState, anchorKey, boxAfter } from "./boxModel";
import { MAX_COMMENTS, type Placement, anchorOf, place, renumbered } from "./comments";
import { type OpenBox, boxAnchor, commentsOf, elsewhere, givesWay, keptBox, sendMessageOf, shownComments, unkept, withFolds } from "./drafts";
import { copy } from "./copy";
import type { AskRequest } from "../../generated/AskRequest";
import type { CiCheck } from "../../generated/CiCheck";
import type { MergeMethod } from "../../generated/MergeMethod";
import type { PullActionKind } from "../../generated/PullActionKind";
import type { PullStatus } from "../../generated/PullStatus";
import type { CheckAction } from "./ChecksList";
import { ask, failureOf, pullAction } from "./githubApi";
import { githubCopy as gc } from "./githubCopy";
import { CreatePullSheet } from "./CreatePullSheet";
import { MergeDialog } from "./MergeDialog";
import { PullBar } from "./PullBar";
import { PushSheet } from "./PushSheet";
import { type BarButton, type BarCommand, type BarMenuItem, announcementsOf, barOf, commandOf } from "./pullModel";
import { recountKey, useGithub } from "./useGithub";
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
  /** Hands the comments' message to the session; the panel clears the comments only when `sent` is true. */
  onSend(text: string, count: number): Promise<SendResult>;
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
export type ChangesHandle = { refresh(): void; review(): void; send(): void; createPull(): void; push(): void; merge(): void; openPull(): void };

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
  const barView = barOf(gh, ghAgent, now);
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

  // Another checkout's comments aren't listed against this one's files (the strip says where they are).
  const comments = useMemo(() => (listed ? shownComments(drafts, listed.folder) : []), [listed, drafts]);
  // The open box's file stays, under "gone", when it leaves the diff, so the box and its text stay too.
  const openPath = open?.state.anchor.path;
  const sections = useMemo(() => (listed ? sectionsOf(listed, drafts?.folds ?? {}, comments, hide, openPath) : []), [listed, drafts, comments, hide, openPath]);
  /** The files of the diff shown now: a fold is kept for these alone. */
  const inDiff = useMemo(() => new Set(listed?.files.map((f) => f.path)), [listed]);
  const placements = useMemo(() => new Map<string, Placement>(comments.map((c) => [c.id, place(c, sections.find((s) => s.path === c.path))])), [sections, comments]);
  const items = useMemo<Item[]>(() => itemsOf(sections, comments, placements), [sections, comments, placements]);
  const draftsRef = useRef(drafts);
  draftsRef.current = drafts;
  const count = drafts?.comments.length ?? 0;
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
    setOpen({ ...next, on: { folder: listed.folder, base: listed.base } });
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
    if (!listed || !d || d.comments.length === 0) return p.announce("There are no comments to send.");
    if (busy) return;
    setBusy(true);
    let result: SendResult;
    try {
      result = await p.onSend(sendMessageOf(d, listed, sections, placements), d.comments.length);
    } finally {
      setBusy(false);
    }
    const latest = draftsRef.current;
    if (result.sent && latest) p.onDrafts(afterSend(latest, result, d.comments));
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
      case "open_on_github":
        return openHttps(item.url);
      case "copy_link":
        return copyOut(item.url ?? "", gc.copiedLink);
    }
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
  const bar = barView === null ? null : <PullBar view={barView} agent={ghAgent} now={now} onMain={onBarMain} onMenu={onBarMenu} onCheck={onBarCheck} onEscape={p.onEscape} />;

  useImperativeHandle(p.ref, () => ({
    refresh: refreshAll,
    review: () => (listed ? p.onReview(listed.base) : p.announce("There are no changes to review.")),
    send: () => void send(),
    createPull: () => barCommand("create"),
    push: () => barCommand("push"),
    merge: () => barCommand("merge"),
    openPull: () => barCommand("open"),
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
    if (item?.kind === "file" || item?.kind === "row") setItemMenu({ key: item.key, anchor, opener });
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
      {itemMenu && menuItem && (menuItem.kind === "file" || menuItem.kind === "row") && (
        <ContextMenu
          label={menuItem.kind === "file" ? "File menu" : "Line menu"}
          items={menuItem.kind === "file" ? headerMenuItems(menuItem.file) : lineMenuItems(menuItem, menuRows(menuItem).length)}
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
        if (drafts) p.onDrafts({ ...drafts, comments: [] });
        p.announce(`Discarded ${commentsOf(count)}.`);
        setBack((n) => n + 1);
      }}
      onKeep={() => {
        setAsking(false);
        setBack((n) => n + 1);
      }}
    />
  );

  if (!listed || sections.length === 0) {
    const empty = <ChangesEmpty reply={reply} noFolder={noFolder} onRetry={refresh} />;
    const around = emptyNotices(listed, drafts);
    if (around === null && bar === null && note === null) {
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
          strip={[...(around ?? []), ...(note === null ? [] : [note])].map((t) => (
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
  const notices = [...noticesOf(listed), ...(away === null ? [] : [away]), ...(note === null ? [] : [note])];
  const boxAt = open === null ? -1 : boxAfter(items, sections, open.state);
  // The key stays the written anchor (the box keeps its text and caret); the label reads the lines it is on now.
  const boxLines = open === null ? null : boxAnchor(open, listed.folder, placeBox(open.state));
  const boxNode =
    open !== null && boxLines !== null && boxAt >= 0
      ? {
          after: boxAt,
          node: <CommentBox key={anchorKey(open.state.anchor)} label={boxLabel(boxLines.start, boxLines.end)} value={open.text} onChange={(text) => setOpen((o) => (o === null ? o : { ...o, text }))} onAction={closeBox} focus={focusBox} />,
        }
      : undefined;
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
      />
      {sendBar}
      {menus}
      {sheetAt}
    </div>
  );
}
