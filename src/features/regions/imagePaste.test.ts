import { describe, expect, it } from "vitest";
import { imageOnlyPaste } from "./imagePaste";

const data = (text: string, types: string[]) => ({ getData: (t: string) => (t === "text/plain" ? text : ""), files: types.map((type) => ({ type })) });

describe("imageOnlyPaste", () => {
  it("takes an image-only paste", () => expect(imageOnlyPaste(data("", ["image/png"]))).toBe(0));
  it("leaves text plus an image as a text paste", () => expect(imageOnlyPaste(data("hello", ["image/png"]))).toBeNull());
  it("takes an image with whitespace-only text", () => expect(imageOnlyPaste(data(" \n\t", ["text/plain", "image/jpeg"]))).toBe(1));
  it("gives null with no files", () => expect(imageOnlyPaste(data("", []))).toBeNull());
  it("gives null for a text/plain file", () => expect(imageOnlyPaste(data("", ["text/plain"]))).toBeNull());
});
