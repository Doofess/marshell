import { describe, expect, it } from "vitest";
import { iconKind } from "./fileIcons";

describe("iconKind", () => {
  it("names map to icons", () => {
    const of = (name: string) => iconKind(name, "file", false);
    expect(of("main.ts")).toBe("code");
    expect(of("README.md")).toBe("markdown");
    expect(of("a.png")).toBe("image");
    expect(of("b.pdf")).toBe("pdf");
    expect(of("i.html")).toBe("web");
    expect(of("d.json")).toBe("data");
    expect(of("e.csv")).toBe("data");
    expect(of(".gitignore")).toBe("config");
    expect(of("x.toml")).toBe("config");
    expect(of("z.zip")).toBe("archive");
    expect(of("notes.txt")).toBe("text");
    expect(of("a.bin")).toBe("other");
    expect(of("noext")).toBe("other");
  });
  it("ignores the case of the extension", () => {
    expect(iconKind("PHOTO.JPG", "file", false)).toBe("image");
  });
  it("a folder is open or closed, a link is a link", () => {
    expect(iconKind("src", "folder", false)).toBe("folder");
    expect(iconKind("src", "folder", true)).toBe("folder-open");
    expect(iconKind("current", "link", false)).toBe("link");
  });
});
