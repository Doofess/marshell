import { useEffect, useMemo, useState } from "react";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { ChangesReply } from "../../generated/ChangesReply";
import { NO_FOLDER_LINE } from "../files/FileTree";
import { absoluteOf } from "../files/treeModel";
import { ChangesIcon, EmptyPanel } from "../pane/EmptyPanel";
import { type ItemRange, DiffList, type ListResult, reach } from "./DiffList";
import { type Placement, place } from "./comments";
import { copy } from "./copy";
import { type Item, itemsOf, sectionsOf } from "./model";
import { useChanges } from "./useChanges";
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
  onSend(text: string, count: number): void;
  onReview(base: string | null): void;
  announce(text: string): void;
  /** The strip's title: "4 files · +120 −18", or "Changes" while there is nothing to count. */
  onSummary(text: string): void;
};

export { listKey } from "./DiffList";

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

/** Where the keyboard is, kept as keys so a poll that moves rows doesn't move it. */
type Keys = { focus: string | null; range: { from: string; to: string } | null };

export function ChangesPanel(p: ChangesPanelProps) {
  const { settings } = useChangesSettings();
  const hide = settings?.hide_whitespace ?? false;
  const wrap = settings?.wrap ?? false;
  const { reply, noFolder, refresh } = useChanges({ session: p.session, shown: p.shown, hideWhitespace: hide });
  const listed = reply?.kind === "listed" ? reply : null;
  const { drafts } = p;

  const sections = useMemo(() => (listed ? sectionsOf(listed, drafts?.folds ?? {}, drafts?.comments ?? [], hide) : []), [listed, drafts, hide]);
  const items = useMemo<Item[]>(() => {
    const comments = drafts?.comments ?? [];
    const placements = new Map<string, Placement>(comments.map((c) => [c.id, place(c, sections.find((s) => s.path === c.path))]));
    return itemsOf(sections, comments, placements);
  }, [sections, drafts]);

  const [keys, setKeys] = useState<Keys>({ focus: null, range: null });
  const found = keys.focus === null ? -1 : items.findIndex((i) => i.key === keys.focus);
  const focus = found >= 0 ? found : 0;
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
    p.onDrafts({ checkout: listed.folder, base: listed.base, comments: drafts?.comments ?? [], folds: { ...drafts?.folds, [path]: folded } });
  };
  const setRange = (next: ItemRange | null) => setKeys((k) => ({ ...k, range: next === null ? null : { from: items[next.from]!.key, to: items[next.to]!.key } }));
  const rowsIn = (r: ItemRange) => items.slice(Math.min(r.from, r.to), Math.max(r.from, r.to) + 1).filter((i) => i.kind === "row").length;

  const onKey = (r: ListResult) => {
    const target = items[r.focus];
    if (target !== undefined) setKeys((k) => ({ focus: target.key, range: r.range === undefined ? k.range : null }));
    if (r.range) {
      setRange(r.range);
      p.announce(copy.selected(rowsIn(r.range)));
    }
    if (r.fold !== undefined) fold(r.fold, true);
    if (r.unfold !== undefined) fold(r.unfold, false);
    // r.open: "comment" is Task 12's box and "menu" is Task 11b's menu.
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

  const onOpenItem = (index: number) => {
    const item = items[index];
    if (item === undefined || !listed || item.kind === "comment" || item.kind === "note" || item.kind === "show") return;
    const windows = listed.folder.includes("\\");
    const path = absoluteOf(listed.folder, item.file.path, windows);
    const line = item.kind === "row" ? (item.file.rows[item.at]?.newNo ?? undefined) : undefined;
    p.onOpen({ path, ...(line === undefined ? {} : { line }), keep: true });
  };

  if (!listed || sections.length === 0) return <ChangesEmpty reply={reply} noFolder={noFolder} onRetry={refresh} />;

  const line = copy.line({ branch: listed.branch, detached: listed.detached, base: listed.base, ahead: listed.ahead, uncommitted: listed.uncommitted, moved: listed.moved, folder: listed.folder });
  const notices = [listed.base === null ? copy.noBase : listed.unrelated ? copy.unrelated(listed.base) : null, listed.more_new > 0 ? copy.moreNew(listed.more_new) : null].filter((t): t is string => t !== null);
  return (
    <div className="changes">
      <div className="changes__strip">
        {line !== "" && <p className="changes__line">{line}</p>}
        {notices.map((t) => (
          <p key={t} className="changes__notice">
            {t}
          </p>
        ))}
      </div>
      <DiffList items={items} focus={focus} range={range} wrap={wrap} onKey={onKey} onClickItem={onClickItem} onOpenItem={onOpenItem} />
    </div>
  );
}
