import { describe, expect, it } from "vitest";
import type { Conversation } from "../../generated/Conversation";
import type { ToolCard } from "../../generated/ToolCard";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { editDiff, filesChanged } from "./changedFiles";

type Edit = { path: string; added?: number; removed?: number; created?: boolean; detail?: string };
const card = (e: Edit): ToolCard => ({
  name: "Edit",
  summary: `Edited ${e.path}`,
  detail: e.detail,
  file: { path: e.path, added: e.added, removed: e.removed, created: e.created ?? false },
  error: false,
});
const conv = (turns: Edit[][]): Conversation => ({
  agent: "claude",
  session_id: "a",
  tab_id: "t1",
  title: "t",
  cwd: "C:/dev",
  limited: false,
  marks: { tags: [], pinned: false, archived: false, bookmarked_turns: [] },
  turns: turns.map((edits, index) => ({ index, at_ms: 0, prompt: "p", items: edits.map((e) => ({ kind: "tool" as const, card: card(e) })) })),
});

describe("filesChanged", () => {
  it("groups two edits to one file and sums their counts", () => {
    const out = filesChanged(conv([[{ path: "src/a.ts", added: 2, removed: 1 }], [], [{ path: "src/a.ts", added: 3, removed: 0 }]]));
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ path: "src/a.ts", added: 5, removed: 1, created: false });
    expect(out[0]!.edits.map((e) => e.turn)).toEqual([0, 2]);
  });

  it("groups a Windows-separator path with its / twin, in order of first edit", () => {
    const out = filesChanged(conv([[{ path: "z.ts", added: 1, removed: 0 }, { path: "src\\a.ts", added: 1, removed: 0 }], [{ path: "src/a.ts", added: 1, removed: 0 }]]));
    expect(out.map((f) => f.path)).toEqual(["z.ts", "src/a.ts"]);
    expect(out[1]!.edits).toHaveLength(2);
  });

  it("gives null for a total when any edit's count is unknown", () => {
    const out = filesChanged(conv([[{ path: "a.ts", added: 2, removed: 1 }, { path: "a.ts" }]]));
    expect(out[0]).toMatchObject({ added: null, removed: null });
  });

  it("is created when the first edit created the file, and skips cards with no file", () => {
    const c = conv([[{ path: "n.md", added: 3, removed: 0, created: true }, { path: "n.md", added: 1, removed: 1 }]]);
    c.turns[0]!.items.push({ kind: "tool", card: { name: "Bash", summary: "Ran x", error: false } });
    const out = filesChanged(c);
    expect(out).toHaveLength(1);
    expect(out[0]!.created).toBe(true);
  });
});

describe("editDiff", () => {
  it("makes a diff that DiffView parses with the right counts", () => {
    const p = parseUnifiedDiff(editDiff("src\\a.ts", card({ path: "src/a.ts", detail: " keep\n-old\n--dashes\n+new\n+more" })));
    expect(p.files).toEqual(["src/a.ts"]);
    expect([p.added, p.removed]).toEqual([2, 2]);
  });

  it("marks every line of a created file as added", () => {
    const p = parseUnifiedDiff(editDiff("n.md", card({ path: "n.md", created: true, detail: "a\n\n-b" })));
    expect([p.added, p.removed]).toEqual([3, 0]);
  });
});
