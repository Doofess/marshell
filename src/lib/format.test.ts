import { describe, expect, it } from "vitest";
import { formatBytes } from "./format";

const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;

describe("formatBytes", () => {
  it("counts in 1024s and labels KB, MB and GB as Windows Explorer does", () => {
    expect(formatBytes(512 * KB)).toBe("512 KB");
    expect(formatBytes(3.4 * MB)).toBe("3.4 MB");
    expect(formatBytes(1.3 * GB)).toBe("1.3 GB");
  });
  it("shows one decimal at MB and GB, even for a whole number", () => {
    expect(formatBytes(5 * MB)).toBe("5.0 MB");
    expect(formatBytes(2 * GB)).toBe("2.0 GB");
  });
  it("shows whole KB, rounded", () => {
    expect(formatBytes(1536)).toBe("2 KB");
    expect(formatBytes(KB)).toBe("1 KB");
  });
  it("shows nothing as 0 KB and anything under 1 KB as 1 KB", () => {
    expect(formatBytes(0)).toBe("0 KB");
    expect(formatBytes(1)).toBe("1 KB");
    expect(formatBytes(1023)).toBe("1 KB");
  });
  it("moves up a unit when rounding would reach 1024", () => {
    expect(formatBytes(MB - 1)).toBe("1.0 MB");
    expect(formatBytes(GB - 1)).toBe("1.0 GB");
  });
  it("keeps GB past a terabyte, and treats a bad number as 0 KB", () => {
    expect(formatBytes(2048 * GB)).toBe("2048.0 GB");
    expect(formatBytes(Number.NaN)).toBe("0 KB");
    expect(formatBytes(-5)).toBe("0 KB");
  });
});
