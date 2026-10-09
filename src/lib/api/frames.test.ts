import { describe, expect, it } from "vitest";
import { decodeServerFrame, encodeAck, encodeBinaryString, encodeInput, encodeResize, encodeResume } from "./frames";

function buf(bytes: number[]): ArrayBuffer {
  return new Uint8Array(bytes).buffer;
}

describe("decodeServerFrame", () => {
  it("decodes output with a u64 big-endian seq", () => {
    const f = decodeServerFrame(buf([1, 0, 0, 0, 0, 0, 0, 1, 2, 104, 105]));
    expect(f).toEqual({ kind: "output", seq: 258, bytes: new Uint8Array([104, 105]) });
  });
  it("decodes reset", () => {
    expect(decodeServerFrame(buf([3, 0, 0, 0, 0, 0, 0, 0, 9, 65]))).toEqual({
      kind: "reset",
      seq: 9,
      screen: new Uint8Array([65]),
    });
  });
  it("decodes a negative exit code", () => {
    expect(decodeServerFrame(buf([2, 255, 255, 255, 255]))).toEqual({ kind: "exit", code: -1 });
  });
  it("rejects short and unknown frames", () => {
    expect(decodeServerFrame(buf([]))).toBeNull();
    expect(decodeServerFrame(buf([1, 0, 0]))).toBeNull();
    expect(decodeServerFrame(buf([2, 0]))).toBeNull();
    expect(decodeServerFrame(buf([99]))).toBeNull();
  });
});

describe("client frames", () => {
  it("encodes input as UTF-8", () => {
    expect(Array.from(encodeInput("é"))).toEqual([0x10, 0xc3, 0xa9]);
  });
  it("encodes xterm binary strings byte for byte", () => {
    expect(Array.from(encodeBinaryString("\x00\xff"))).toEqual([0x10, 0x00, 0xff]);
  });
  it("encodes resize as JSON", () => {
    const f = encodeResize({ cols: 120, rows: 40 });
    expect(f[0]).toBe(0x11);
    expect(new TextDecoder().decode(f.slice(1))).toBe('{"cols":120,"rows":40}');
  });
  it("encodes ack and resume as u64 big-endian", () => {
    expect(Array.from(encodeAck(258))).toEqual([0x12, 0, 0, 0, 0, 0, 0, 1, 2]);
    expect(Array.from(encodeResume(0))).toEqual([0x13, 0, 0, 0, 0, 0, 0, 0, 0]);
  });
});
