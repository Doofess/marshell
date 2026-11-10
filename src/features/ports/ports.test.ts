import { describe, expect, it } from "vitest";
import type { PortChip } from "../../generated/PortChip";
import { chipLabel, chipMenuItems, isLoopbackUrl } from "./ports";
import { openChipIn } from "./ports";

const chip = (port: number, url = `http://localhost:${port}/`): PortChip => ({ port, url });

describe("isLoopbackUrl", () => {
  it("refuses_a_non_loopback_url", () => {
    for (const u of ["http://localhost:3000@evil.example/", "http://evil.example/", "http://localhost.evil.example:80/", "http://192.168.1.5:8787/", "file:///c:/x", "javascript:alert(1)", "http://user:pw@localhost:3000/", "ftp://localhost/", "not a url", ""])
      expect(isLoopbackUrl(u), u).toBe(false);
  });
  it("accepts localhost, 127.0.0.1 and [::1] over http or https", () => {
    for (const u of ["http://localhost:5173/", "https://127.0.0.1:8443/x", "http://[::1]:3000/"]) expect(isLoopbackUrl(u), u).toBe(true);
  });
});

describe("chipLabel", () => {
  it("names the host and the port", () => {
    expect(chipLabel(chip(5173))).toBe("localhost:5173");
    expect(chipLabel(chip(9099, "http://127.0.0.1:9099/"))).toBe("127.0.0.1:9099");
    expect(chipLabel(chip(3000, "http://[::1]:3000/"))).toBe("[::1]:3000");
  });
  it("falls back to localhost when the URL does not parse", () => {
    expect(chipLabel(chip(80, "garbage"))).toBe("localhost:80");
  });
});

describe("chipMenuItems", () => {
  it("offers Open in browser and Copy URL for one chip", () => {
    expect(chipMenuItems([chip(5173)])).toEqual([
      { id: "open:0", label: "Open in browser" },
      { id: "copy:0", label: "Copy URL" },
    ]);
  });
  it("lists an Open item per chip for the +N menu", () => {
    expect(chipMenuItems([chip(1), chip(2)]).map((i) => ("label" in i ? i.label : ""))).toEqual(["Open localhost:1", "Open localhost:2"]);
  });
});

describe("a chip's menu where browser tabs are available", () => {
  it("keeps its rows and gains Open in the system browser", () => {
    expect(chipMenuItems([chip(5173)], true)).toEqual([
      { id: "open:0", label: "Open in browser" },
      { id: "copy:0", label: "Copy URL" },
      { id: "system:0", label: "Open in the system browser" },
    ]);
  });
  it("gives the +N menu a system-browser row per chip after its Open rows", () => {
    expect(chipMenuItems([chip(1), chip(2)], true).map((i) => ("label" in i ? i.label : ""))).toEqual(["Open localhost:1", "Open localhost:2", "Open localhost:1 in the system browser", "Open localhost:2 in the system browser"]);
  });
  it("is unchanged where they aren't", () => {
    expect(chipMenuItems([chip(5173)], false)).toEqual(chipMenuItems([chip(5173)]));
  });
});

describe("openChipIn", () => {
  it("opens a loopback chip in the side pane when it can, and never a non-loopback one", async () => {
    const pane: string[] = [];
    const system: string[] = [];
    const openPage = (u: string) => (pane.push(u), true);
    const openSystem = async (u: string) => void system.push(u);
    await openChipIn("http://localhost:5173/", openPage, openSystem);
    await openChipIn("http://evil.example/", openPage, openSystem);
    expect(pane).toEqual(["http://localhost:5173/"]);
    expect(system).toEqual([]);
  });
  it("opens the system browser where there is no side pane to open in", async () => {
    const system: string[] = [];
    await openChipIn("http://localhost:5173/", null, async (u) => void system.push(u));
    expect(system).toEqual(["http://localhost:5173/"]);
  });
});
