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
const handlers = { onOutput: vi.fn(), onReset: vi.fn(), onLive: vi.fn(), onExit: vi.fn() };
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

describe("PtySocket for an administrator session", () => {
  const sink = () => ({ input: vi.fn().mockResolvedValue(undefined), resize: vi.fn().mockResolvedValue(undefined) });

  it("elevated_rows_send_input_through_invoke_not_the_socket", () => {
    const trusted = sink();
    const socket = new PtySocket(endpoint, "tab", handlers, trusted);
    const ws = FakeWebSocket.instances[0]!;
    ws.open();
    socket.sendInput("ls");
    socket.sendBytes("");
    expect(trusted.input).toHaveBeenCalledWith("ls");
    expect(opcodes(ws)).toEqual([FRAME.RESUME]);
    socket.close();
  });

  it("resize_of_an_elevated_row_goes_through_invoke", () => {
    const trusted = sink();
    const socket = new PtySocket(endpoint, "tab", handlers, trusted);
    const ws = FakeWebSocket.instances[0]!;
    socket.resize(100, 30);
    ws.open();
    socket.resize(120, 40);
    expect(trusted.resize).toHaveBeenCalledWith(100, 30);
    expect(trusted.resize).toHaveBeenLastCalledWith(120, 40);
    expect(opcodes(ws)).toEqual([FRAME.RESUME]);
    socket.close();
  });

  it("a_plain_row_still_uses_the_socket", () => {
    const socket = new PtySocket(endpoint, "tab", handlers);
    const ws = FakeWebSocket.instances[0]!;
    ws.open();
    socket.sendInput("ls");
    socket.resize(100, 30);
    expect(opcodes(ws)).toEqual([FRAME.RESUME, FRAME.INPUT, FRAME.RESIZE]);
    socket.close();
  });

  it("switches to the window's path when a row turns out to be elevated, and sends the known size once", () => {
    const trusted = sink();
    const socket = new PtySocket(endpoint, "tab", handlers);
    socket.resize(90, 20);
    socket.setTrusted(trusted);
    expect(trusted.resize).toHaveBeenCalledWith(90, 20);
    socket.setTrusted(null);
    socket.sendInput("a");
    expect(trusted.input).not.toHaveBeenCalled();
    socket.close();
  });

  it("an_invoke_error_shows_the_core_copy: a refused keystroke reaches onInputError with the sentence", async () => {
    const onInputError = vi.fn();
    const trusted = { input: vi.fn().mockRejectedValue(new Error("No such session.")), resize: vi.fn() };
    const socket = new PtySocket(endpoint, "tab", { ...handlers, onInputError }, trusted);
    socket.sendInput("a");
    await vi.advanceTimersByTimeAsync(0);
    expect(onInputError).toHaveBeenCalledWith("No such session.");
    socket.close();
  });
});
