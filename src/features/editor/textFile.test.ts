import { describe, expect, it } from "vitest";
import { decodeFile, encodeFile, encodingLabel, eolLabel } from "./textFile";

const buf = (...n: number[]) => new Uint8Array(n);
const utf8 = (s: string) => new TextEncoder().encode(s);
const same = (a: Uint8Array, b: Uint8Array) => expect(Array.from(a)).toEqual(Array.from(b));

/** UTF-16 bytes built by hand, with an optional BOM. */
function utf16(s: string, le: boolean, bom: boolean): Uint8Array {
  const out: number[] = bom ? (le ? [0xff, 0xfe] : [0xfe, 0xff]) : [];
  for (let i = 0; i < s.length; i++) {
    const u = s.charCodeAt(i);
    out.push(...(le ? [u & 0xff, u >> 8] : [u >> 8, u & 0xff]));
  }
  return new Uint8Array(out);
}

function roundTrip(bytes: Uint8Array) {
  const d = decodeFile(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer);
  expect(d.kind).toBe("text");
  if (d.kind !== "text") return d;
  same(encodeFile(d.text, d.encoding, d.eol), bytes);
  return d;
}

const ab = (b: Uint8Array) => b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer;

describe("decodeFile and encodeFile", () => {
  it("roundTrips_plain_utf8", () => {
    const d = roundTrip(utf8("héllo\nworld\n"));
    expect(d).toEqual({ kind: "text", text: "héllo\nworld\n", encoding: "utf-8", eol: "lf" });
  });
  it("roundTrips_a_bom", () => {
    const d = roundTrip(new Uint8Array([0xef, 0xbb, 0xbf, ...utf8("hi\n")]));
    expect(d).toEqual({ kind: "text", text: "hi\n", encoding: "utf-8-bom", eol: "lf" });
  });
  it("roundTrips_utf16_le_and_be", () => {
    expect(roundTrip(utf16("a\nb\n", true, true))).toMatchObject({ text: "a\nb\n", encoding: "utf-16le" });
    expect(roundTrip(utf16("a\nb\n", false, true))).toMatchObject({ text: "a\nb\n", encoding: "utf-16be" });
  });
  it("roundTrips_crlf", () => {
    expect(roundTrip(utf8("a\r\nb\r\n"))).toMatchObject({ text: "a\r\nb\r\n", eol: "crlf" });
  });
  it("roundTrips_no_final_newline", () => {
    expect(roundTrip(utf8("a\nb"))).toMatchObject({ text: "a\nb" });
  });
  it("roundTrips_an_empty_file", () => {
    expect(roundTrip(buf())).toEqual({ kind: "text", text: "", encoding: "utf-8", eol: "lf" });
  });
  it("roundTrips_a_bom_alone", () => {
    expect(roundTrip(buf(0xef, 0xbb, 0xbf))).toMatchObject({ text: "", encoding: "utf-8-bom" });
    expect(roundTrip(buf(0xff, 0xfe))).toMatchObject({ text: "", encoding: "utf-16le" });
  });
  it("mixed_endings_take_the_kind_there_is_most_of", () => {
    const d = decodeFile(ab(utf8("a\r\nb\r\nc\nd")));
    expect(d).toMatchObject({ kind: "text", eol: "crlf" });
    if (d.kind !== "text") return;
    const out = encodeFile(d.text, d.encoding, d.eol);
    expect(new TextDecoder().decode(out).split("\r\n")).toHaveLength(4);
    expect(new TextDecoder().decode(out).replace(/\r\n/g, "")).not.toMatch(/[\r\n]/);
  });
  it("a_lone_cr_becomes_the_files_ending", () => {
    expect(new TextDecoder().decode(encodeFile("a\rb\nc", "utf-8", "lf"))).toBe("a\nb\nc");
    expect(new TextDecoder().decode(encodeFile("a\rb\nc", "utf-8", "crlf"))).toBe("a\r\nb\r\nc");
  });
  it("binary_a_nul_in_utf8", () => {
    expect(decodeFile(ab(buf(104, 105, 0, 33)))).toEqual({ kind: "binary" });
  });
  it("binary_invalid_utf8", () => {
    expect(decodeFile(ab(buf(0x41, 0xff, 0x42)))).toEqual({ kind: "binary" });
  });
  it("binary_utf16_with_an_odd_byte", () => {
    expect(decodeFile(ab(buf(0xff, 0xfe, 0x41)))).toEqual({ kind: "binary" });
  });
  it("binary_utf16_with_a_lone_surrogate", () => {
    expect(decodeFile(ab(buf(0xff, 0xfe, 0x00, 0xd8, 0x41, 0x00)))).toEqual({ kind: "binary" });
    expect(decodeFile(ab(buf(0xfe, 0xff, 0xdc, 0x00)))).toEqual({ kind: "binary" });
  });
  it("a_nul_in_utf16_text_is_text", () => {
    expect(decodeFile(ab(utf16("a", true, true)))).toMatchObject({ kind: "text", text: "a" });
  });
  it("labels", () => {
    expect(["utf-8", "utf-8-bom", "utf-16le", "utf-16be"].map((e) => encodingLabel(e as never))).toEqual(["UTF-8", "UTF-8 with BOM", "UTF-16 LE", "UTF-16 BE"]);
    expect([eolLabel("lf"), eolLabel("crlf")]).toEqual(["LF", "CRLF"]);
  });
});
