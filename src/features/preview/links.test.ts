import { describe, expect, it } from "vitest";
import { classifyLink } from "./links";

const doc = "C:\\p\\docs\\a.md";
const none = { kind: "none" };

describe("classifyLink", () => {
  it("takes #id as an anchor in the same document", () => {
    expect(classifyLink("#setup", doc)).toEqual({ kind: "anchor", id: "setup" });
    expect(classifyLink("#caf%C3%A9", doc)).toEqual({ kind: "anchor", id: "café" });
    expect(classifyLink("#", doc)).toEqual(none);
  });
  it("resolves a relative path against the document's folder", () => {
    expect(classifyLink("b.md", doc)).toEqual({ kind: "local", path: "C:\\p\\docs\\b.md" });
    expect(classifyLink("./sub/c.md", doc)).toEqual({ kind: "local", path: "C:\\p\\docs\\sub\\c.md" });
    expect(classifyLink("../src/x.rs#L12", doc)).toEqual({ kind: "local", path: "C:\\p\\src\\x.rs", line: 12 });
    expect(classifyLink("..\\src\\x.rs#L3-L9", doc)).toEqual({ kind: "local", path: "C:\\p\\src\\x.rs", line: 3 });
  });
  it("never climbs above the root", () => {
    expect(classifyLink("../../../../x.md", doc)).toEqual({ kind: "local", path: "C:\\x.md" });
  });
  it("keeps an absolute local path, with either separator", () => {
    expect(classifyLink("C:\\q\\r.md", doc)).toEqual({ kind: "local", path: "C:\\q\\r.md" });
    expect(classifyLink("D:/q/r.md", doc)).toEqual({ kind: "local", path: "D:\\q\\r.md" });
    expect(classifyLink("/q/r.md", doc)).toEqual({ kind: "local", path: "C:\\q\\r.md" });
  });
  it("uses the document's separator for a posix document", () => {
    expect(classifyLink("../b.md", "/home/me/docs/a.md")).toEqual({ kind: "local", path: "/home/me/b.md" });
    expect(classifyLink("/etc/x", "/home/me/a.md")).toEqual({ kind: "local", path: "/etc/x" });
  });
  it("decodes percent escapes and ignores a fragment that is not a line", () => {
    expect(classifyLink("my%20notes.md#top", doc)).toEqual({ kind: "local", path: "C:\\p\\docs\\my notes.md" });
    expect(classifyLink("bad%zz.md", doc)).toEqual({ kind: "local", path: "C:\\p\\docs\\bad%zz.md" });
  });
  it("takes http and https as web, case-insensitively, normalised through URL", () => {
    expect(classifyLink("https://example.com/x", doc)).toEqual({ kind: "web", url: "https://example.com/x" });
    expect(classifyLink("HTTP://EXAMPLE.com", doc)).toEqual({ kind: "web", url: "http://example.com/" });
    expect(classifyLink("https://bücher.example/a", doc)).toEqual({ kind: "web", url: "https://xn--bcher-kva.example/a" });
  });
  it("refuses every other scheme", () => {
    for (const href of ["javascript:alert(1)", "JaVaScRiPt:alert(1)", "mailto:a@b", "file:///C:/x", "data:text/html,hi", "blob:https://x/1", "vbscript:x", "tel:1", "ftp://x/y", "C:foo"]) {
      expect(classifyLink(href, doc), href).toEqual(none);
    }
  });
  it("refuses a UNC or device path, however it is written", () => {
    for (const href of ["\\\\evil\\share\\a.txt", "//evil/share/a.txt", "\\\\?\\C:\\x", "\\\\.\\pipe\\x", "%5C%5Cevil%5Cshare", "%2F%2Fevil/share"]) {
      expect(classifyLink(href, doc), href).toEqual(none);
    }
  });
  it("refuses an empty href and control characters", () => {
    expect(classifyLink("", doc)).toEqual(none);
    expect(classifyLink("   ", doc)).toEqual(none);
    expect(classifyLink("a%00b.md", doc)).toEqual(none);
  });
});
