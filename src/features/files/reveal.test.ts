import { describe, expect, it, vi } from "vitest";
import { revealFile, revealLabel, revealRefused } from "./reveal";

describe("reveal", () => {
  it("revealLabel per system", () => {
    expect(revealLabel("windows")).toBe("Show in File Explorer");
    expect(revealLabel("mac")).toBe("Show in Finder");
    expect(revealLabel("linux")).toBe("Show in file manager");
  });
  it("revealFile calls reveal with the path and announces a failure", async () => {
    const reveal = vi.fn().mockResolvedValue(undefined);
    const say = vi.fn();
    await revealFile("C:\\p\\a.txt", say, reveal);
    expect(reveal).toHaveBeenCalledWith("C:\\p\\a.txt");
    expect(say).not.toHaveBeenCalled();

    const failing = vi.fn().mockRejectedValue(new Error("no such file"));
    await revealFile("C:\\p\\b.txt", say, failing);
    expect(say).toHaveBeenCalledWith("Couldn't show the file: no such file");
    await revealFile("C:\\p\\c.txt", say, vi.fn().mockRejectedValue("denied"));
    expect(say).toHaveBeenLastCalledWith("Couldn't show the file: denied");
  });
  it("refuses mixed slashes at the start", () => {
    expect(revealRefused("/\\srv/x")).toBe(true);
    expect(revealRefused("\\/srv/x")).toBe(true);
    expect(revealRefused("C:/x")).toBe(false);
  });
  it("revealFile refuses a network path without calling reveal", async () => {
    for (const path of ["\\\\server\\share\\a.txt", "//server/a.txt"]) {
      const reveal = vi.fn();
      const say = vi.fn();
      expect(revealRefused(path)).toBe(true);
      await revealFile(path, say, reveal);
      expect(reveal).not.toHaveBeenCalled();
      expect(say).toHaveBeenCalledWith("Couldn't show the file: it's on a network share.");
    }
    expect(revealRefused("C:\\a\\b.txt")).toBe(false);
    expect(revealRefused("/home/a/b.txt")).toBe(false);
  });
});
