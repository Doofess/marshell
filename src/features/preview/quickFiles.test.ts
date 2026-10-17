import { describe, expect, it, vi } from "vitest";
import { cachedFileList, folderName } from "./quickFiles";

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
