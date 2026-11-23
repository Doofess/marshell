import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { PullThreads } from "../../generated/PullThreads";
import type { BarMenuItem, BarView } from "./pullModel";
import { threadCopy } from "./threadCopy";
import { toResolve } from "./threads";
import { addUnresolved } from "./threadsSend";

export type ThreadsBar = { resolve: { label: string; first: string } | null; talk: { label: string } | null; add: { label: string } | null };

/** The pull request bar's part of GitHub's comments: what to resolve, the reviews and comments, and how many unresolved ones "add all" would add. */
export function threadsBarOf(threads: PullThreads | null, drafts: ChangesDrafts | undefined): ThreadsBar {
  if (threads === null) return { resolve: null, talk: null, add: null };
  const open = toResolve(threads.threads);
  const { added, sent } = addUnresolved(drafts, { folder: "", base: null }, threads);
  return {
    resolve: open.length === 0 ? null : { label: threadCopy.toResolve(open.length), first: open[0]!.id },
    talk: threads.notes.length === 0 ? null : { label: threadCopy.talk(threads.notes.length) },
    add: added === 0 ? null : { label: sent === 0 ? threadCopy.addAll(added) : threadCopy.addSome(added, sent) },
  };
}

/** The bar with the two buttons and, in its menu before "Open on GitHub", the row `add_unresolved`. */
export function withThreads(view: BarView, t: ThreadsBar): BarView {
  if (view.scope !== "pull") return view;
  const menu = [...view.menu];
  if (t.add !== null) {
    const row: BarMenuItem = { id: "add_unresolved", label: t.add.label, disabled: null };
    const at = menu.findIndex((m) => m.id === "open_on_github");
    menu.splice(at < 0 ? menu.length : at, 0, row);
  }
  return { ...view, menu, resolve: t.resolve === null ? null : { label: t.resolve.label }, talk: t.talk };
}

const keysOf = (r: PullThreads) => [...r.threads.flatMap((t) => t.comments.map((c) => ({ key: `c${c.id}`, author: c.author }))), ...r.notes.map((n) => ({ key: `n${n.id}`, author: n.author }))];

/** "2 new comments on #7." for comments (in threads and notes) that `before` didn't hold and the viewer didn't write. Null on the first reply, for another pull request, and for none. */
export function newCommentsOf(before: PullThreads | null, after: PullThreads): string | null {
  if (before === null || before.number !== after.number) return null;
  const known = new Set(keysOf(before).map((k) => k.key));
  const fresh = keysOf(after).filter((k) => !known.has(k.key) && k.author !== after.viewer).length;
  return fresh === 0 ? null : threadCopy.newComments(fresh, after.number);
}
