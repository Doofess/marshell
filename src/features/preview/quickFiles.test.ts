import { describe, expect, it, vi } from "vitest";
import { buildQuickFiles, cachedFileList, folderName } from "./quickFiles";

const list = (root: string) => ({ root, files: ["a.ts"], truncated: false });

describe("cachedFileList", () => {
  it("asks once within 10 seconds, and again after", async () => {
    const fetch = vi.fn(async (tab: string) => list(`/r/${tab}`));
    let t = 1000;
    const cache = new Map();
    await cachedFileList("a", fetch, () => t, cache);
    t += 9000;
    await cachedFileList("a", fetch, () => t, cache);
    expect(fetch).toHaveBeenCalledTimes(1);
    t += 2000;
    await cachedFileList("a", fetch, () => t, cache);
    await cachedFileList("b", fetch, () => t, cache);
    expect(fetch).toHaveBeenCalledTimes(3);
  });
  it("does not remember a failure", async () => {
    const fetch = vi.fn().mockRejectedValueOnce(new Error("down")).mockResolvedValueOnce(list("/r"));
    const cache = new Map();
    await expect(cachedFileList("a", fetch, () => 0, cache)).rejects.toThrow("down");
    expect((await cachedFileList("a", fetch, () => 0, cache)).root).toBe("/r");
  });
});

describe("folderName", () => {
  it("is the last part of the root", () => {
    expect(folderName("C:\\dev\\Marshell\\")).toBe("Marshell");
    expect(folderName("/home/me/proj")).toBe("proj");
  });
});

describe("buildQuickFiles", () => {
  const seenAbs = ["C:\\r\\sub\\a.ts", "C:\\elsewhere\\b.ts"];
  it("shows the seen paths at once, as given, while the list loads", () => {
    const q = buildQuickFiles("loading", seenAbs);
    expect(q.files.status).toBe("loading");
    expect(q.files.list).toEqual([]);
    expect(q.files.seen).toHaveLength(2);
    expect(q.pathOf(q.files.seen[1]!)).toBe(seenAbs[1]);
  });
  it("is an error without inventing a folder", () => {
    expect(buildQuickFiles("error", []).files).toMatchObject({ status: "error", list: [], seen: [], folder: "" });
  });
  it("is ready with the list, seen paths inside the root shown relative", () => {
    const q = buildQuickFiles({ root: "C:\\r", files: ["a.ts"], truncated: true }, seenAbs);
    expect(q.files).toMatchObject({ status: "ready", folder: "r", list: ["a.ts"], truncated: true });
    expect(q.files.seen[0]).toBe("sub/a.ts");
    expect(q.pathOf("sub/a.ts")).toBe(seenAbs[0]);
  });
});
