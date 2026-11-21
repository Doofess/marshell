import { beforeEach, describe, expect, it, vi } from "vitest";

const asked: string[] = [];
vi.mock("./browserApi", async (original) => {
  const real = await original<typeof import("./browserApi")>();
  return {
    ...real,
    browser: {
      ...real.browser,
      close: (key: string) => {
        asked.push(`close ${key}`);
        return Promise.resolve();
      },
      open: (key: string, url: string) => {
        asked.push(`open ${key} ${url}`);
        return Promise.resolve();
      },
    },
  };
});

import { reloadPages } from "./useBrowserPages";

describe("reloadPages", () => {
  beforeEach(() => {
    asked.length = 0;
  });

  // Found in review: private pages share one cookie jar, which ends only when the last of them closes (S14). Closing and
  // opening one page at a time kept a private page alive throughout, so "Clear browsing data…" under "Never keep" cleared nothing.
  it("closes every page before it opens any, so the private pages' shared data ends in between", () => {
    const opened = reloadPages([
      { key: "a/t1", url: "https://one.example/" },
      { key: "a/t2", url: null },
      { key: "b/t1", url: "https://two.example/" },
    ]);
    expect(opened).toBe(2);
    expect(asked).toEqual(["close a/t1", "close a/t2", "close b/t1", "open a/t1 https://one.example/", "open b/t1 https://two.example/"]);
  });

  it("asks nothing for an empty plan", () => {
    expect(reloadPages([])).toBe(0);
    expect(asked).toEqual([]);
  });
});
