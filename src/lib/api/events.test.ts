import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TOKEN_SUBPROTOCOL_PREFIX, WS_SUBPROTOCOL } from "../../generated/constants";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";
import { EventSocket } from "./events";

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  constructor(
    public url: string,
    public protocols: string[],
  ) {
    FakeWebSocket.instances.push(this);
  }
  close(): void {
    this.onclose?.();
  }
  open(): void {
    this.onopen?.();
  }
  receive(data: unknown): void {
    this.onmessage?.({ data });
  }
}

const endpoint: Endpoint = { port: 7, token: "t0k", platform: "windows" };
const snapshot = (names: string[]): SessionsSnapshot => ({
  sessions: names.map((name, i) => ({
    tab_id: `tab-${name}`,
    number: i + 1,
    name,
    agent: null,
    profile_id: "p",
    cwd: "C:/x",
    state: { kind: "running" },
    command: { kind: "unknown" },
  })),
  closed: [],
});
const frame = (s: SessionsSnapshot) => JSON.stringify({ type: "sessions", data: s });

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("EventSocket", () => {
  it("connects to /v1/events with the token subprotocol", () => {
    const socket = new EventSocket(endpoint, vi.fn());
    const ws = FakeWebSocket.instances[0]!;
    expect(ws.url).toBe("ws://127.0.0.1:7/v1/events");
    expect(ws.protocols).toEqual([WS_SUBPROTOCOL, `${TOKEN_SUBPROTOCOL_PREFIX}t0k`]);
    socket.close();
  });

  it("delivers snapshot messages", () => {
    const onSnapshot = vi.fn();
    const socket = new EventSocket(endpoint, onSnapshot);
    const ws = FakeWebSocket.instances[0]!;
    ws.open();
    ws.receive(frame(snapshot(["a"])));
    ws.receive(frame(snapshot(["a", "b"])));

    expect(onSnapshot).toHaveBeenCalledTimes(2);
    expect(onSnapshot.mock.calls[1]![0]).toEqual(snapshot(["a", "b"]));
    socket.close();
  });

  it("replaces the list on reconnect", () => {
    const onSnapshot = vi.fn();
    const socket = new EventSocket(endpoint, onSnapshot);
    const first = FakeWebSocket.instances[0]!;
    first.open();
    first.receive(frame(snapshot(["a", "b"])));

    first.close();
    expect(FakeWebSocket.instances).toHaveLength(1);
    vi.advanceTimersByTime(250);
    expect(FakeWebSocket.instances).toHaveLength(2);
    const second = FakeWebSocket.instances[1]!;
    second.open();
    second.receive(frame(snapshot(["c"])));

    expect(onSnapshot).toHaveBeenCalledTimes(2);
    expect(onSnapshot.mock.calls[1]![0]).toEqual(snapshot(["c"]));
    socket.close();
  });

  it("backs off 250 ms doubling up to 5 s, and resets on open", () => {
    const socket = new EventSocket(endpoint, vi.fn());
    const delays = [250, 500, 1000, 2000, 4000, 5000, 5000];
    delays.forEach((delay, i) => {
      FakeWebSocket.instances[i]!.close();
      vi.advanceTimersByTime(delay - 1);
      expect(FakeWebSocket.instances).toHaveLength(i + 1);
      vi.advanceTimersByTime(1);
      expect(FakeWebSocket.instances).toHaveLength(i + 2);
    });
    const last = FakeWebSocket.instances.at(-1)!;
    last.open();
    last.close();
    vi.advanceTimersByTime(250);
    expect(FakeWebSocket.instances).toHaveLength(delays.length + 2);
    socket.close();
  });

  it("ignores unknown message types and bad data", () => {
    const onSnapshot = vi.fn();
    const socket = new EventSocket(endpoint, onSnapshot);
    const ws = FakeWebSocket.instances[0]!;
    ws.open();
    ws.receive(JSON.stringify({ type: "other", data: snapshot(["a"]) }));
    ws.receive("not json");
    ws.receive(new ArrayBuffer(4));
    ws.receive(JSON.stringify({ type: "sessions" }));

    expect(onSnapshot).not.toHaveBeenCalled();
    socket.close();
  });

  it("does not reconnect after close()", () => {
    const socket = new EventSocket(endpoint, vi.fn());
    socket.close();
    vi.advanceTimersByTime(10_000);
    expect(FakeWebSocket.instances).toHaveLength(1);
  });

  it("delivers nothing after close(), even from a late message", () => {
    const onSnapshot = vi.fn();
    const socket = new EventSocket(endpoint, onSnapshot);
    const ws = FakeWebSocket.instances[0]!;
    ws.open();
    socket.close();
    // A real socket in CLOSING can still deliver a frame; receive() does not fire onclose.
    ws.receive(frame(snapshot(["late"])));

    expect(onSnapshot).not.toHaveBeenCalled();
  });
});
