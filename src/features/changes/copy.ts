/** Every string the Changes panel shows that isn't a file's own text. Numbers read as "1,234". */
const n = (v: number) => v.toLocaleString("en-US");
const files = (v: number) => `${n(v)} ${v === 1 ? "file" : "files"}`;
const commits = (v: number) => `${n(v)} ${v === 1 ? "commit" : "commits"}`;

export type LineInfo = {
  branch: string | null;
  detached: string | null;
  base: string | null;
  ahead: number;
  uncommitted: boolean;
  moved: boolean;
  folder: string;
};

export const copy = {
  /** "4 files · +120 −18" (the minus sign is U+2212), or "No changes". */
  summary(fileCount: number, added: number, removed: number): string {
    return fileCount === 0 ? "No changes" : `${files(fileCount)} · +${n(added)} \u2212${n(removed)}`;
  },
  /** "feat/login · 3 commits ahead of origin/main · uncommitted work · in C:\\other". A part with nothing to say is left out. */
  line(i: LineInfo): string {
    const parts: string[] = [];
    const name = i.branch ?? (i.detached === null ? null : `Detached at ${i.detached}`);
    if (name !== null) parts.push(name);
    if (i.ahead > 0 && i.base !== null) parts.push(`${commits(i.ahead)} ahead of ${i.base}`);
    if (i.uncommitted) parts.push("uncommitted work");
    if (i.moved) parts.push(`in ${i.folder}`);
    return parts.join(" · ");
  },
  noBase: "No main branch found. Showing uncommitted work.",
  unrelated: (base: string) => `No common history with ${base}. Showing uncommitted work.`,
  emptyRepo: (folder: string) => ({ title: "No git repository", line: `${folder} isn't in a git checkout.` }),
  emptyNone: (base: string | null) => ({ title: "No changes", line: base === null ? "Nothing is uncommitted." : `Nothing differs from ${base}.` }),
  emptyFailed: (reason: string) => ({ title: "Can't read the changes", line: reason, action: "Try again" }),
  cut: (count: number) => `The diff is cut at 4 MiB. ${n(count)} more ${count === 1 ? "file isn't" : "files aren't"} shown.`,
  moreNew: (count: number) => `And ${n(count)} more new ${count === 1 ? "file isn't" : "files aren't"} shown.`,
  notes: {
    binary: "Binary file, not shown.",
    large: "New file over 256 KiB, not shown.",
    whitespace: "Only whitespace changed.",
    gone: "No changes in this file now.",
  },
  /** What a screen reader hears when Shift+Up or Down widens the rows. */
  selected: (count: number) => `${n(count)} ${count === 1 ? "line" : "lines"} selected`,
  /** What a comment says once the lines it sits on changed. */
  changed: (range: boolean) => (range ? "The lines changed" : "The line changed"),
} as const;
