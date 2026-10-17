import { describe, expect, it, vi } from "vitest";
import { findPathCandidates } from "../preview/pathCandidates";
import { logicalLine, pathLinkProvider, previewHint, type BufferLike } from "./pathLinks";

/**
 * A buffer of rows. A row starting with `!` is a wrapped continuation. In a row, `世` and `界` are wide (width 2) and are
 * followed by `~`, their width-0 second cell; `\0` is a blank cell.
 */
function stub(rows: string[]): BufferLike {
  return {
    getLine: (y) => {
      const raw = rows[y];
      if (raw === undefined) return undefined;
      const isWrapped = raw.startsWith("!");
      const cells = Array.from(isWrapped ? raw.slice(1) : raw).map((ch): [string, number] => {
        if (ch === "~") return ["", 0];
        if (ch === "\0") return ["", 1];
        return [ch, ch === "世" || ch === "界" ? 2 : 1];
      });
      return {
        isWrapped,
        length: cells.length,
        getCell: (x) => {
          const cell = cells[x];
          return cell ? { getChars: () => cell[0], getWidth: () => cell[1] } : undefined;
        },
      };
    },
  };
}

describe("logicalLine", () => {
  it("maps a path that wraps onto the next row to cells on both rows", () => {
    // Row 3 (index 2) ends with "src/ma", row 4 (index 3) continues with "in.rs:5 ok".
    const buf = stub(["prompt", "other", "see src/ma", "!in.rs:5 ok"]);
    const ll = logicalLine(buf, 3); // from the continuation row it finds the same logical line
    expect(ll.text).toBe("see src/main.rs:5 ok");
    const c = findPathCandidates(ll.text)[0]!;
    expect(c.text).toBe("src/main.rs");
    expect(ll.cellOf(c.start)).toMatchObject({ x: 5, y: 3 });
    expect(ll.cellOf(c.end - 1)).toMatchObject({ x: 7, y: 4 }); // the "5" of ":5"
    expect(logicalLine(buf, 2).text).toBe(ll.text); // and from its first row
  });

  it("starts a new logical line at a row that is not wrapped", () => {
    const buf = stub(["abc", "def", "!ghi"]);
    expect(logicalLine(buf, 0).text).toBe("abc");
    expect(logicalLine(buf, 1).text).toBe("defghi");
  });

  it("gives a wide character two cells, shifting what follows it", () => {
    // "世" takes x 1-2, the space is x 3, so "src/a.rs" starts at x 4.
    const ll = logicalLine(stub(["世~ src/a.rs"]), 0);
    expect(ll.text).toBe("世 src/a.rs");
    const c = findPathCandidates(ll.text)[0]!;
    expect(c.start).toBe(2);
    expect(ll.cellOf(c.start)).toMatchObject({ x: 4, y: 1 });
    expect(ll.cellOf(0)).toMatchObject({ x: 1, y: 1, width: 2 });
  });

  it("maps both halves of a surrogate pair to its one cell, and a blank cell to a space", () => {
    const ll = logicalLine(stub(["😀b"]), 0);
    expect(ll.text).toBe("😀b");
    expect(ll.cellOf(0)).toMatchObject({ x: 1, y: 1 });
    expect(ll.cellOf(1)).toMatchObject({ x: 1, y: 1 });
    expect(ll.cellOf(2)).toMatchObject({ x: 2, y: 1 });
    expect(logicalLine(stub(["a\0b"]), 0).text).toBe("a b");
  });
});

describe("pathLinkProvider", () => {
  const term = (rows: string[]) => ({ buffer: { active: stub(rows) }, element: undefined });
  const run = (p: ReturnType<typeof pathLinkProvider>, y: number) => new Promise<Parameters<Parameters<typeof p.provideLinks>[1]>[0]>((r) => p.provideLinks(y, r));

  it("links only the paths that resolve, with a range over their cells (1-based, end inclusive)", async () => {
    const resolve = vi.fn(async (_tab: string, c: string[]) => c.map((t) => (t === "src/a.rs" ? "C:\\p\\src\\a.rs" : null)));
    const p = pathLinkProvider(term(["世~ src/a.rs:4 nope.txt"]), "t1", "windows", vi.fn(), resolve);
    const links = (await run(p, 1)) ?? [];
    expect(resolve).toHaveBeenCalledTimes(1);
    expect(resolve).toHaveBeenCalledWith("t1", ["src/a.rs", "nope.txt"]);
    expect(links).toHaveLength(1);
    expect(links[0]!.text).toBe("src/a.rs");
    expect(links[0]!.range).toEqual({ start: { x: 4, y: 1 }, end: { x: 13, y: 1 } });
  });

  it("asks once for a candidate it has seen, and again after 5 s", async () => {
    let t = 1000;
    const resolve = vi.fn(async (_tab: string, c: string[]) => c.map(() => "/p/a.rs"));
    const p = pathLinkProvider(term(["a.rs"]), "t1", "linux", vi.fn(), resolve, () => t);
    await run(p, 1);
    await run(p, 1);
    expect(resolve).toHaveBeenCalledTimes(1);
    t += 5001;
    await run(p, 1);
    expect(resolve).toHaveBeenCalledTimes(2);
  });

  it("opens the preview on Ctrl+click (Cmd on macOS), not on a plain click", async () => {
    const open = vi.fn();
    const resolve = async (_tab: string, c: string[]) => c.map(() => "/p/a.rs");
    const lin = pathLinkProvider(term(["a.rs:3:2"]), "t1", "linux", open, resolve);
    const [l] = (await run(lin, 1)) ?? [];
    l!.activate({ ctrlKey: false, metaKey: false } as MouseEvent, l!.text);
    l!.activate({ ctrlKey: false, metaKey: true } as MouseEvent, l!.text);
    expect(open).not.toHaveBeenCalled();
    l!.activate({ ctrlKey: true, metaKey: false } as MouseEvent, l!.text);
    expect(open).toHaveBeenCalledWith({ path: "/p/a.rs", line: 3, col: 2 });

    const mac = vi.fn();
    const [m] = (await run(pathLinkProvider(term(["a.rs"]), "t1", "macos", mac, resolve), 1)) ?? [];
    m!.activate({ ctrlKey: true, metaKey: false } as MouseEvent, m!.text);
    expect(mac).not.toHaveBeenCalled();
    m!.activate({ ctrlKey: false, metaKey: true } as MouseEvent, m!.text);
    expect(mac).toHaveBeenCalledWith({ path: "/p/a.rs" });
  });

  it("gives no links when resolving fails, and sends at most 64 candidates", async () => {
    const down = vi.fn(async () => {
      throw new Error("core down");
    });
    expect(await run(pathLinkProvider(term(["a.rs"]), "t1", "linux", vi.fn(), down), 1)).toBeUndefined();
    const many = Array.from({ length: 80 }, (_, i) => `f${i}.rs`).join(" ");
    const resolve = vi.fn(async (_tab: string, c: string[]) => c.map(() => null));
    await run(pathLinkProvider(term([many]), "t1", "linux", vi.fn(), resolve), 1);
    expect(resolve.mock.calls[0]?.[1]).toHaveLength(64);
  });

  it("words the hint for the platform", () => {
    expect(previewHint("windows")).toBe("Ctrl+click to preview");
    expect(previewHint("macos")).toBe("⌘+click to preview");
  });
});
