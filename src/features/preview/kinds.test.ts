import { describe, expect, it } from "vitest";
import { decodeText, kindOf, languageOf, splitPath } from "./kinds";

describe("kindOf", () => {
  it("reads the kind from the extension, whatever its case", () => {
    expect(kindOf("a.RS")).toBe("code");
    expect(kindOf("C:\\src\\main.tsx")).toBe("code");
    expect(kindOf("README.md")).toBe("markdown");
    expect(kindOf("x.htm")).toBe("html");
    expect(kindOf("p.svg")).toBe("image");
    expect(kindOf("p.PNG")).toBe("image");
    expect(kindOf("d.pdf")).toBe("pdf");
  });
  it("calls a file with no known extension text, and leaves the bytes to say it isn't", () => {
    expect(kindOf("Makefile")).toBe("text");
    expect(kindOf("notes.txt")).toBe("text");
    expect(kindOf("a.unknownext")).toBe("text");
    expect(kindOf("dir.d/file")).toBe("text");
  });
});

describe("languageOf", () => {
  it("names the Shiki language of a code file", () => {
    expect(languageOf("a.tsx")).toBe("tsx");
    expect(languageOf("lib.RS")).toBe("rust");
    expect(languageOf("x.py")).toBe("python");
    expect(languageOf("README.md")).toBe("markdown");
    expect(languageOf("index.html")).toBe("html");
  });
  it("is undefined for plain text", () => {
    expect(languageOf("Makefile")).toBeUndefined();
    expect(languageOf("notes.txt")).toBeUndefined();
  });
});

describe("splitPath", () => {
  it("gives the file name and its folder, for either separator", () => {
    expect(splitPath("C:\\dev\\Airport\\src\\a.rs")).toEqual({ name: "a.rs", folder: "C:\\dev\\Airport\\src" });
    expect(splitPath("/home/me/a.rs")).toEqual({ name: "a.rs", folder: "/home/me" });
    expect(splitPath("a.rs")).toEqual({ name: "a.rs", folder: "" });
  });
});

describe("decodeText", () => {
  const bytes = (s: string) => new TextEncoder().encode(s).buffer as ArrayBuffer;
  it("decodes UTF-8", () => {
    expect(decodeText(bytes("héllo\n"))).toEqual({ kind: "text", text: "héllo\n" });
  });
  it("is not text with a NUL byte in the first 8 KB", () => {
    expect(decodeText(new Uint8Array([104, 105, 0, 33]).buffer)).toEqual({ kind: "binary" });
  });
  it("lets a NUL after the first 8 KB through, if the bytes still decode", () => {
    const b = new Uint8Array(8200).fill(97);
    b[8195] = 0;
    expect(decodeText(b.buffer).kind).toBe("text");
  });
  it("is not text when the bytes are not valid UTF-8", () => {
    expect(decodeText(new Uint8Array([0xff, 0xfe, 0x41]).buffer)).toEqual({ kind: "binary" });
  });
});
