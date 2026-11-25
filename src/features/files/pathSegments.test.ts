import { describe, expect, it } from "vitest";
import { pathAtEnd, pathSegments } from "./pathSegments";

const win = { windows: true, home: "C:\\Users\\Home Office" };

describe("pathSegments", () => {
  it("a file in the project reads root, folders, file, and each folder reveals itself", () => {
    expect(pathSegments("C:\\dev\\Airport", "C:\\dev\\Airport\\src\\a\\b.ts", win)).toEqual([
      { label: "Airport", folder: "" },
      { label: "src", folder: "src" },
      { label: "a", folder: "src/a" },
      { label: "b.ts", folder: null },
    ]);
  });
  it("ignores case on Windows and a trailing separator on the root", () => {
    expect(pathSegments("c:\\dev\\Airport\\", "C:\\DEV\\airport\\x.ts", win).map((s) => s.label)).toEqual(["Airport", "x.ts"]);
  });
  it("a file under home starts at ~ and reveals nothing", () => {
    expect(pathSegments("C:\\dev\\Airport", "C:\\Users\\Home Office\\notes\\n.md", win)).toEqual([
      { label: "~", folder: null },
      { label: "notes", folder: null },
      { label: "n.md", folder: null },
    ]);
  });
  it("a file elsewhere starts at its drive", () => {
    expect(pathSegments("C:\\dev\\Airport", "D:\\x\\y.txt", win)).toEqual([
      { label: "D:", folder: null },
      { label: "x", folder: null },
      { label: "y.txt", folder: null },
    ]);
  });
  it("outside Windows, a path elsewhere starts at /", () => {
    expect(pathSegments("/p/r", "/etc/hosts", { windows: false, home: "/home/u" }).map((s) => s.label)).toEqual(["/", "etc", "hosts"]);
    expect(pathSegments("/p/r", "/p/r/a/b", { windows: false, home: "/home/u" }).map((s) => s.folder)).toEqual(["", "a", null]);
  });
  it("with no file the bar shows the root alone", () => {
    expect(pathSegments("C:\\dev\\Airport", null, win)).toEqual([{ label: "Airport", folder: "" }]);
    expect(pathSegments(null, null, win)).toEqual([]);
  });
});

describe("pathAtEnd", () => {
  it("tells a path at its end from one the person scrolled back (pre-merge review 6w: a resize threw the scrolled place away)", () => {
    expect(pathAtEnd({ scrollLeft: 321, clientWidth: 182, scrollWidth: 503 })).toBe(true);
    expect(pathAtEnd({ scrollLeft: 120, clientWidth: 182, scrollWidth: 503 })).toBe(false);
    expect(pathAtEnd({ scrollLeft: 0, clientWidth: 182, scrollWidth: 503 })).toBe(false);
  });
  it("counts a path that fits as at its end, and allows a fraction of a pixel", () => {
    expect(pathAtEnd({ scrollLeft: 0, clientWidth: 300, scrollWidth: 300 })).toBe(true);
    expect(pathAtEnd({ scrollLeft: 320.4, clientWidth: 182, scrollWidth: 503 })).toBe(true);
  });
  it("reads a right-to-left path, whose end is a negative scroll", () => {
    expect(pathAtEnd({ scrollLeft: -321, clientWidth: 182, scrollWidth: 503 })).toBe(true);
    expect(pathAtEnd({ scrollLeft: -100, clientWidth: 182, scrollWidth: 503 })).toBe(false);
  });
});
