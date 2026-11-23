import { type Ref, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { ContextMenu } from "../../components/ContextMenu/ContextMenu";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { ChangesReply } from "../../generated/ChangesReply";
import { NO_FOLDER_LINE } from "../files/FileTree";
import { absoluteOf } from "../files/treeModel";
import { ChangesIcon, EmptyPanel } from "../pane/EmptyPanel";
import { type ItemRange, DiffList, type ListResult, reach } from "./DiffList";
import { type ChangesChoice, changesChoice, changesMenuItems, headerMenuItems, lineMenuItems, openLineOf, settingLabel } from "./changesMenu";
import { type BoxAction, CommentBox, boxLabel } from "./CommentBox";
import { type BoxState, anchorKey, boxAfter } from "./boxModel";
import { MAX_COMMENTS, type Placement, anchorOf, place, renumbered } from "./comments";
import { commentsOf, elsewhere, sendMessageOf, shownComments, unkept, withFolds } from "./drafts";
import { copy } from "./copy";
import { type Item, type Row, type Section, cutFiles, focusIndex, itemsOf, sectionsOf } from "./model";
import { reviewLabel } from "./review";
import { SendBar } from "./SendBar";
import { type SendResult, afterSend } from "./send";
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
};

export { listKey } from "./DiffList";

/** What the palette's Changes commands call on the mounted panel. Each announces why when it can't act. */
export type ChangesHandle = { refresh(): void; review(): void; send(): void };


/** The panel's empty states, each with its mark; only a failed read has an action. */
export function ChangesEmpty({ reply, noFolder, onRetry }: { reply: ChangesReply | null; noFolder: boolean; onRetry(): void }) {
  // Nothing read yet: the panel stays blank until the first reply.
  if (reply === null && !noFolder) return null;
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

/** The lines under the strip's line: no base or unrelated histories, the new files past 200, and the files a cut diff leaves out. */
export function noticesOf(reply: ChangesReply): string[] {
  const cut = cutFiles(reply);
  return [
    reply.base === null ? copy.noBase : reply.unrelated ? copy.unrelated(reply.base) : null,
    reply.more_new > 0 ? copy.moreNew(reply.more_new) : null,
    cut > 0 ? copy.cut(cut) : null,
  ].filter((t): t is string => t !== null);
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

  // The comment box (its text lives here, so a poll never loses it), the Send bar's question and a send under way.
  const [open, setOpen] = useState<{ state: BoxState; text: string } | null>(null);
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);

  // Another checkout's comments aren't listed against this one's files (the strip says where they are).
  const comments = useMemo(() => (listed ? shownComments(drafts, listed.folder) : []), [listed, drafts]);
  // The open box's file stays, under "gone", when it leaves the diff, so the box and its text stay too.
  const openPath = open?.state.anchor.path;
  const sections = useMemo(() => (listed ? sectionsOf(listed, drafts?.folds ?? {}, comments, hide, openPath) : []), [listed, drafts, comments, hide, openPath]);
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
      sections.filter((s) => s.note !== "gone").length,
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
    p.onDrafts(withFolds(drafts, listed, { [path]: folded }));
  };
  const setRange = (next: ItemRange | null) => setKeys((k) => ({ ...k, range: next === null ? null : { from: items[next.from]!.key, to: items[next.to]!.key } }));
  const rowsIn = (r: ItemRange) => items.slice(Math.min(r.from, r.to), Math.max(r.from, r.to) + 1).filter((i) => i.kind === "row").length;

  useEffect(() => {
    if (count === 0) setAsking(false);
  }, [count]);

  // The keyboard goes back to its item once the box or the bar is gone (never because a poll changed the list).
  useEffect(() => {
    if (back > 0) box.current?.querySelector<HTMLElement>(`[data-changes-index="${focus}"]`)?.focus();
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
  const startBox = (item: Item | undefined) => {
    if (!listed || item === undefined) return;
    if (item.kind === "comment") {
      const { id, text, ...anchor } = item.comment;
      focusBox.current = true;
      setOpen({ state: { anchor, id }, text });
      return;
    }
    if (item.kind !== "row") return;
    const held = heldItems(item).filter((i): i is Extract<Item, { kind: "row" }> => i.kind === "row");
    const rows = held.map((i) => i.file.rows[i.at]!);
    if (held.length === 0 || rows.some((r) => r.kind === "hunk")) return;
    const away = elsewhere(drafts, listed.folder) ?? unkept(item.file.path);
    if (away !== null) return p.announce(away);
    if (count >= MAX_COMMENTS) return p.announce(`A session holds at most ${MAX_COMMENTS} comments.`);
    const ats = held.map((i) => i.at);
    focusBox.current = true;
    setOpen({ state: { anchor: anchorOf(item.file, Math.min(...ats), Math.max(...ats)), id: null }, text: "" });
  };

  /** Where the box's rows are now. */
  const placeBox = (state: BoxState): Placement =>
    place({ ...state.anchor, id: "", text: "" }, sections.find((s) => s.path === state.anchor.path));

  const closeBox = (action: BoxAction) => {
    const current = open;
    setOpen(null);
    setBack((n) => n + 1);
    if (!current || !listed) return;
    const text = current.text;
    if (action === "close" || text.trim() === "") return;
    const comments = drafts?.comments ?? [];
    const { id } = current.state;
    // Saved at the lines its rows are on now, which a reply during the writing may have moved.
    const anchor = renumbered(current.state.anchor, placeBox(current.state));
    const next = id === null ? [...comments, { ...anchor, id: crypto.randomUUID(), text }] : comments.map((c) => (c.id === id ? { ...renumbered(c, placements.get(c.id)), text } : c));
    p.onDrafts({ checkout: id === null ? listed.folder : (drafts?.checkout ?? listed.folder), base: id === null ? listed.base : (drafts?.base ?? listed.base), comments: next, folds: drafts?.folds ?? {} });
    p.announce(action === "add" ? `Comment added. ${next.length} to send.` : "Comment kept as a draft.");
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
    if (result.sent && latest) p.onDrafts(afterSend(latest, result));
  };

  useImperativeHandle(p.ref, () => ({
    refresh: refresh,
    review: () => (listed ? p.onReview(listed.base) : p.announce("There are no changes to review.")),
    send: () => void send(),
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
  const onOpenItem = (index: number) => {
    const item = items[index];
    if (item === undefined || !listed || item.kind === "comment" || item.kind === "note" || item.kind === "show") return;
    openFile(item.file, item.kind === "row" ? (item.file.rows[item.at]?.newNo ?? undefined) : undefined);
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
    p.onDrafts(withFolds(drafts, listed, all));
  };
  const runMenu = (id: string) => {
    const choice: ChangesChoice | null = changesChoice(id);
    const opener = p.menu?.opener;
    p.onMenuClose();
    opener?.focus();
    if (choice === null) return;
    if (choice.kind === "refresh") refresh();
    else if (choice.kind === "review") p.onReview(listed?.base ?? null);
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
          items={changesMenuItems({ settings, failed, review: p.agent === null ? null : reviewLabel(p.agent, p.agentName), hasFiles: sections.some((s) => s.rows.length > 0) })}
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

  if (!listed || sections.length === 0) {
    return (
      <>
        <ChangesEmpty reply={reply} noFolder={noFolder} onRetry={refresh} />
        {menus}
      </>
    );
  }

  const line = copy.line({ branch: listed.branch, detached: listed.detached, base: listed.base, ahead: listed.ahead, uncommitted: listed.uncommitted, moved: listed.moved, folder: listed.folder });
  const away = elsewhere(drafts, listed.folder);
  const notices = away === null ? noticesOf(listed) : [...noticesOf(listed), away];
  const boxAt = open === null ? -1 : boxAfter(items, sections, open.state);
  // The key stays the written anchor (the box keeps its text and caret); the label reads the lines it is on now.
  const boxLines = open === null ? null : renumbered(open.state.anchor, placeBox(open.state));
  const boxNode =
    open !== null && boxLines !== null && boxAt >= 0
      ? {
          after: boxAt,
          node: <CommentBox key={anchorKey(open.state.anchor)} label={boxLabel(boxLines.start, boxLines.end)} value={open.text} onChange={(text) => setOpen((o) => (o === null ? o : { ...o, text }))} onAction={closeBox} focus={focusBox} />,
        }
      : undefined;
  return (
    <div className="changes" ref={box}>
      <div className="changes__strip">
        {line !== "" && <p className="changes__line">{line}</p>}
        {notices.map((t) => (
          <p key={t} className="changes__notice">
            {t}
          </p>
        ))}
      </div>
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
      {count > 0 && (
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
      )}
      {menus}
    </div>
  );
}
