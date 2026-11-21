import { describe, expect, it } from "vitest";
import { pathSegments } from "./pathSegments";

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
