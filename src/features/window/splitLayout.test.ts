import { describe, expect, it } from "vitest";
import { ARRANGEMENTS, CELL, HANDLE, HIT, MIN_USABLE, PANE_LIMIT, chooseArrangement, fitsPanes, maxPanes, paneCells, planFor, scoreArrangement, tracks, type Arrangement } from "./splitLayout";

describe("split limits", () => {
  it("shows at most four terminals at once", () => {
    expect(PANE_LIMIT).toBe(4);
    expect(Object.keys(ARRANGEMENTS).map(Number)).toEqual([1, 2, 3, 4]);
  });
  it("offers more than one way to arrange two, three and four, and exactly one for a single pane", () => {
    expect(ARRANGEMENTS[1]).toEqual(["single"]);
    for (const n of [2, 3, 4] as const) expect(ARRANGEMENTS[n].length, String(n)).toBeGreaterThan(1);
  });
});

describe("planFor", () => {
  it("gives every pane its own cell inside the grid, with no two cells overlapping", () => {
    for (const n of [1, 2, 3, 4] as const)
      for (const a of ARRANGEMENTS[n]) {
        const p = planFor(n, a);
        expect(p.cells.length, `${n} ${a}`).toBe(n);
        const taken = new Set<string>();
        for (const c of p.cells)
          for (let col = c.colStart; col < c.colEnd; col++)
            for (let row = c.rowStart; row < c.rowEnd; row++) {
              const key = `${col},${row}`;
              expect(taken.has(key), `${n} ${a} overlaps at ${key}`).toBe(false);
              taken.add(key);
            }
        // Every grid line is covered, either by a pane or by a handle track.
        const handleCells = p.handles.flatMap((h) => {
          const out: string[] = [];
          for (let col = h.colStart; col < h.colEnd; col++) for (let row = h.rowStart; row < h.rowEnd; row++) out.push(`${col},${row}`);
          return out;
        });
        for (const k of handleCells) expect(taken.has(k), `${n} ${a} handle sits on a pane at ${k}`).toBe(false);
      }
  });
  it("puts a separator between every pair of neighbouring panes: panes minus one for a line, two more for a grid", () => {
    expect(planFor(1, "single").handles).toHaveLength(0);
    expect(planFor(2, "columns").handles).toHaveLength(1);
    expect(planFor(3, "columns").handles).toHaveLength(2);
    expect(planFor(3, "main-left").handles).toHaveLength(2);
    expect(planFor(4, "grid").handles).toHaveLength(2);
    expect(planFor(4, "rows").handles).toHaveLength(3);
  });
  it("makes a main pane the full height of its side: main-left is one tall pane beside two stacked", () => {
    const p = planFor(3, "main-left");
    const [main, a, b] = p.cells;
    expect(main).toMatchObject({ colStart: 1, colEnd: 2, rowStart: 1, rowEnd: 4 });
    expect(a!.colStart).toBe(b!.colStart);
    expect(a!.rowEnd).toBeLessThanOrEqual(b!.rowStart);
  });
  it("tells a handle which way it resizes", () => {
    expect(planFor(2, "columns").handles[0]!.orientation).toBe("vertical");
    expect(planFor(2, "rows").handles[0]!.orientation).toBe("horizontal");
  });
  it("rejects an arrangement that does not suit the count", () => {
    expect(() => planFor(2, "grid")).toThrow();
    expect(() => planFor(5 as never, "columns")).toThrow();
  });
});

describe("tracks", () => {
  it("lays pane tracks out with a 1 px hairline track between them", () => {
    expect(tracks(2, 0.5)).toBe("minmax(0, 0.5fr) 1px minmax(0, 0.5fr)");
    expect(tracks(1, 0.5)).toBe("minmax(0, 1fr)");
    expect(tracks(3, 0.5)).toBe("minmax(0, 1fr) 1px minmax(0, 1fr) 1px minmax(0, 1fr)");
  });
  it("spends one pixel of the window on a divider and gives the other seven of the 8 px hit area to the panes' edges", () => {
    expect(HANDLE).toBe(1);
    expect(HIT).toBe(8);
  });
});

describe("paneCells: how many columns and rows each pane really gets", () => {
  it("counts the terminal's padding and the 28 px pane header, and the handle track", () => {
    const [only] = paneCells(planFor(1, "single"), { w: 816, h: 540 });
    expect(only).toEqual({ cols: Math.floor((816 - CELL.padX) / CELL.w), rows: Math.floor((540 - CELL.header - CELL.padY) / CELL.h) });
    const two = paneCells(planFor(2, "columns"), { w: 816, h: 540 });
    expect(two[0]!.cols).toBe(Math.floor(((816 - HANDLE) / 2 - CELL.padX) / CELL.w));
  });
  it("follows the ratio, and cannot go below a sliver", () => {
    const wide = paneCells(planFor(2, "columns"), { w: 1000, h: 600 }, { col: 0.7 });
    expect(wide[0]!.cols).toBeGreaterThan(wide[1]!.cols);
    const clamped = paneCells(planFor(2, "columns"), { w: 1000, h: 600 }, { col: 0.01 });
    expect(clamped[0]!.cols).toBe(paneCells(planFor(2, "columns"), { w: 1000, h: 600 }, { col: 0.2 })[0]!.cols);
  });
  it("gives the main pane of main-left the whole height", () => {
    const [main, a] = paneCells(planFor(3, "main-left"), { w: 1200, h: 800 });
    expect(main!.rows).toBeGreaterThan(a!.rows);
    expect(main!.cols).toBe(a!.cols);
  });
});

describe("chooseArrangement: the window space is used so every pane stays usable", () => {
  const pick = (n: 1 | 2 | 3 | 4, w: number, h: number): Arrangement => chooseArrangement(n, { w, h });
  it("puts two terminals side by side in a wide window, stacked in a tall one", () => {
    expect(pick(2, 1400, 800)).toBe("columns");
    expect(pick(2, 560, 1100)).toBe("rows");
  });
  it("makes a two by two grid of four in a normal window", () => {
    expect(pick(4, 1400, 800)).toBe("grid");
  });
  it("lines four up in a very tall narrow one, where a grid would leave each pane too narrow", () => {
    expect(pick(4, 420, 1500)).toBe("rows");
  });
  it("prefers one big pane beside two small ones for three, unless that clearly starves a pane", () => {
    expect(pick(3, 1600, 800)).toBe("main-left");
  });
  it("stacks three when the window is narrow", () => {
    expect(pick(3, 520, 1300)).toBe("rows");
  });
  it("has one answer for one", () => {
    expect(pick(1, 800, 600)).toBe("single");
  });
  it("never picks an arrangement that suits another count", () => {
    for (const n of [1, 2, 3, 4] as const)
      for (const [w, h] of [[720, 440], [1000, 700], [1920, 1000], [500, 900]] as const) expect(ARRANGEMENTS[n]).toContain(pick(n, w, h));
  });
});

describe("how many panes a window can usefully hold", () => {
  it("calls a pane usable from 60 columns by 14 rows", () => {
    expect(MIN_USABLE).toEqual({ cols: 60, rows: 14 });
  });
  it("holds one pane in the smallest window, where two would each be too narrow or too short", () => {
    expect(maxPanes({ w: 668, h: 344 })).toBe(1);
  });
  it("holds two where they can stack with room, three or four only in a roomy window", () => {
    expect(maxPanes({ w: 892, h: 664 })).toBe(2);
    expect(maxPanes({ w: 1212, h: 500 })).toBe(2);
    expect(maxPanes({ w: 1212, h: 724 })).toBe(4);
    expect(maxPanes({ w: 1920, h: 1000 })).toBe(4);
  });
  it("never offers more than four, or fewer than one", () => {
    expect(maxPanes({ w: 5000, h: 5000 })).toBe(PANE_LIMIT);
    expect(maxPanes({ w: 0, h: 0 })).toBe(1);
  });
  it("is contiguous: every smaller count also fits whenever a count is offered", () => {
    for (const w of [600, 900, 1200, 1500, 1800, 2400])
      for (const h of [300, 500, 700, 900, 1200]) {
        const max = maxPanes({ w, h });
        for (let n = 1; n <= max; n++) expect(fitsPanes(n as 1 | 2 | 3 | 4, { w, h }), `${n} in ${w}x${h}`).toBe(true);
        if (max < PANE_LIMIT) expect(fitsPanes((max + 1) as 1 | 2 | 3 | 4, { w, h }), `${max + 1} in ${w}x${h}`).toBe(false);
      }
  });
  it("fits a count when at least one arrangement gives every pane the minimum", () => {
    expect(fitsPanes(2, { w: 892, h: 664 })).toBe(true);
    expect(fitsPanes(3, { w: 892, h: 664 })).toBe(false);
  });
});

describe("chooseArrangement never picks a layout that starves a pane when another would not", () => {
  it("stacks two in 892 by 666: side by side would leave 53 columns, under the usable minimum", () => {
    expect(chooseArrangement(2, { w: 892, h: 666 })).toBe("rows");
  });
  it("keeps every pane at 60 by 14 or more whenever any arrangement can", () => {
    for (const n of [2, 3, 4] as const)
      for (const w of [700, 892, 1100, 1212, 1500, 1920])
        for (const h of [400, 520, 666, 724, 900, 1100]) {
          const size = { w, h };
          if (!fitsPanes(n, size)) continue;
          const cells = paneCells(planFor(n, chooseArrangement(n, size)), size);
          for (const c of cells) {
            expect(c.cols, `${n} in ${w}x${h}`).toBeGreaterThanOrEqual(MIN_USABLE.cols);
            expect(c.rows, `${n} in ${w}x${h}`).toBeGreaterThanOrEqual(MIN_USABLE.rows);
          }
        }
  });
});

describe("scoreArrangement", () => {
  it("is 1 when every pane has at least 80 columns and 24 rows, and falls as the smallest pane shrinks", () => {
    expect(scoreArrangement(planFor(2, "columns"), { w: 1800, h: 1000 })).toBe(1);
    const small = scoreArrangement(planFor(2, "columns"), { w: 600, h: 400 });
    expect(small).toBeLessThan(1);
    expect(small).toBeGreaterThan(0);
  });
});
