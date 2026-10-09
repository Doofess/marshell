import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FRAME } from "../../generated/constants";
import type { Endpoint } from "../../generated/Endpoint";
import { PtySocket } from "./ptySocket";

/** Records sends; the test opens it by hand. */
class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  readyState = FakeWebSocket.CONNECTING;
  binaryType = "blob";
  sent: Uint8Array[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  constructor(
    public url: string,
    public protocols: string[],
  ) {
    FakeWebSocket.instances.push(this);
  }
  send(data: Uint8Array): void {
    this.sent.push(data);
  }
  close(): void {
    this.readyState = FakeWebSocket.CLOSED;
    this.onclose?.();
  }
  open(): void {
    this.readyState = FakeWebSocket.OPEN;
    this.onopen?.();
  }
}

const endpoint: Endpoint = { port: 1, token: "t", platform: "windows" };
const handlers = { onOutput: vi.fn(), onReset: vi.fn(), onExit: vi.fn() };
const opcodes = (ws: FakeWebSocket) => ws.sent.map((f) => f[0]);
const resizeJson = (f: Uint8Array) => JSON.parse(new TextDecoder().decode(f.subarray(1)));

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("PtySocket", () => {
  it("replays a resize made before open as RESUME then RESIZE", () => {
    const socket = new PtySocket(endpoint, "tab", handlers);
    const ws = FakeWebSocket.instances[0]!;
    socket.resize(146, 40);
    expect(ws.sent).toHaveLength(0);

    ws.open();
    expect(opcodes(ws)).toEqual([FRAME.RESUME, FRAME.RESIZE]);
    expect(resizeJson(ws.sent[1]!)).toEqual({ cols: 146, rows: 40 });
    socket.close();
  });

  it("sends only RESUME on open when no size is known", () => {
    const socket = new PtySocket(endpoint, "tab", handlers);
    const ws = FakeWebSocket.instances[0]!;
    ws.open();
    expect(opcodes(ws)).toEqual([FRAME.RESUME]);
    socket.close();
  });

  it("replays the latest size after a reconnect", () => {
    const socket = new PtySocket(endpoint, "tab", handlers);
    const first = FakeWebSocket.instances[0]!;
    first.open();
    socket.resize(100, 30);
    expect(opcodes(first)).toEqual([FRAME.RESUME, FRAME.RESIZE]);

    first.close();
    socket.resize(120, 35); // while reconnecting: dropped on the wire, remembered
    vi.runOnlyPendingTimers();
    const second = FakeWebSocket.instances[1]!;
    second.open();
    expect(opcodes(second)).toEqual([FRAME.RESUME, FRAME.RESIZE]);
    expect(resizeJson(second.sent[1]!)).toEqual({ cols: 120, rows: 35 });
    socket.close();
  });
});
