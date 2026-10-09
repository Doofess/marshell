import { FRAME } from "../../generated/constants";
import type { Resize } from "../../generated/Resize";

export type ServerFrame =
  | { kind: "output"; seq: number; bytes: Uint8Array }
  | { kind: "reset"; seq: number; screen: Uint8Array }
  | { kind: "exit"; code: number };

export function decodeServerFrame(buf: ArrayBuffer): ServerFrame | null {
  if (buf.byteLength < 1) return null;
  const view = new DataView(buf);
  const op = view.getUint8(0);
  if (op === FRAME.OUTPUT || op === FRAME.RESET) {
    if (buf.byteLength < 9) return null;
    const seq = Number(view.getBigUint64(1));
    const bytes = new Uint8Array(buf, 9);
    return op === FRAME.OUTPUT ? { kind: "output", seq, bytes } : { kind: "reset", seq, screen: bytes };
  }
  if (op === FRAME.EXIT) {
    if (buf.byteLength < 5) return null;
    return { kind: "exit", code: view.getInt32(1) };
  }
  return null;
}

// Encoders return ArrayBuffer-backed arrays: WebSocket.send() rejects Uint8Array<ArrayBufferLike> under TS >= 5.9.
function withOp(op: number, payload: Uint8Array): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(1 + payload.length);
  out[0] = op;
  out.set(payload, 1);
  return out;
}

export function encodeInput(data: string | Uint8Array): Uint8Array<ArrayBuffer> {
  return withOp(FRAME.INPUT, typeof data === "string" ? new TextEncoder().encode(data) : data);
}

/** xterm's onBinary gives a string where each char is one byte (e.g. mouse reports). */
export function encodeBinaryString(data: string): Uint8Array<ArrayBuffer> {
  return withOp(FRAME.INPUT, Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff));
}

export function encodeResize(size: Resize): Uint8Array<ArrayBuffer> {
  return withOp(FRAME.RESIZE, new TextEncoder().encode(JSON.stringify(size)));
}

function encodeU64(op: number, n: number): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(9);
  const view = new DataView(out.buffer);
  view.setUint8(0, op);
  view.setBigUint64(1, BigInt(n));
  return out;
}

export const encodeAck = (n: number) => encodeU64(FRAME.ACK, n);
export const encodeResume = (n: number) => encodeU64(FRAME.RESUME, n);
