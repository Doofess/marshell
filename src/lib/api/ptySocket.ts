import { TOKEN_SUBPROTOCOL_PREFIX, WS_SUBPROTOCOL } from "../../generated/constants";
import type { Endpoint } from "../../generated/Endpoint";
import type { Resize } from "../../generated/Resize";
import { decodeServerFrame, encodeAck, encodeBinaryString, encodeInput, encodeResize, encodeResume } from "./frames";

export interface PtyHandlers {
  onOutput(seq: number, bytes: Uint8Array): void;
  onReset(seq: number, screen: Uint8Array): void;
  onExit(code: number): void;
}

const ACK_EVERY_BYTES = 64 * 1024;
const ACK_DELAY_MS = 16;

/** One terminal's connection to the core. Reconnects with backoff and resumes where the renderer left off. */
export class PtySocket {
  private ws: WebSocket | null = null;
  private processedUpTo = 0;
  // The renderer's size, replayed on every open: resizes while CONNECTING would otherwise be dropped.
  private lastSize: Resize | null = null;
  private lastAckSent = 0;
  private ackTimer: ReturnType<typeof setTimeout> | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private retries = 0;
  private done = false;

  constructor(
    private readonly endpoint: Endpoint,
    private readonly tabId: string,
    private readonly handlers: PtyHandlers,
  ) {
    this.connect();
  }

  private connect(): void {
    if (this.done) return;
    const url = `ws://127.0.0.1:${this.endpoint.port}/v1/pty/${encodeURIComponent(this.tabId)}`;
    const ws = new WebSocket(url, [WS_SUBPROTOCOL, TOKEN_SUBPROTOCOL_PREFIX + this.endpoint.token]);
    ws.binaryType = "arraybuffer";
    ws.onopen = () => {
      this.retries = 0;
      ws.send(encodeResume(this.processedUpTo));
      if (this.lastSize) ws.send(encodeResize(this.lastSize));
    };
    ws.onmessage = (ev) => {
      if (!(ev.data instanceof ArrayBuffer)) return;
      const frame = decodeServerFrame(ev.data);
      if (!frame) return;
      if (frame.kind === "output") this.handlers.onOutput(frame.seq, frame.bytes);
      else if (frame.kind === "reset") this.handlers.onReset(frame.seq, frame.screen);
      else {
        this.done = true;
        this.handlers.onExit(frame.code);
      }
    };
    ws.onclose = () => {
      this.ws = null;
      if (this.done) return;
      const delay = Math.min(2000, 100 * 2 ** this.retries++);
      this.retryTimer = setTimeout(() => {
        this.retryTimer = null;
        this.connect();
      }, delay);
    };
    this.ws = ws;
  }

  private send(frame: Uint8Array<ArrayBuffer>): void {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(frame);
  }

  sendInput(data: string): void {
    this.send(encodeInput(data));
  }

  sendBytes(binary: string): void {
    this.send(encodeBinaryString(binary));
  }

  resize(cols: number, rows: number): void {
    if (cols <= 0 || rows <= 0) return;
    this.lastSize = { cols, rows };
    this.send(encodeResize(this.lastSize));
  }

  /** The renderer has fully processed output before this byte offset. Acks are batched. */
  processed(upTo: number): void {
    if (this.done) return;
    this.processedUpTo = upTo;
    if (upTo - this.lastAckSent >= ACK_EVERY_BYTES) {
      this.flushAck();
    } else if (this.ackTimer === null) {
      this.ackTimer = setTimeout(() => this.flushAck(), ACK_DELAY_MS);
    }
  }

  private flushAck(): void {
    if (this.ackTimer !== null) clearTimeout(this.ackTimer);
    this.ackTimer = null;
    this.lastAckSent = this.processedUpTo;
    this.send(encodeAck(this.processedUpTo));
  }

  close(): void {
    this.done = true;
    if (this.ackTimer !== null) clearTimeout(this.ackTimer);
    this.ackTimer = null;
    if (this.retryTimer !== null) clearTimeout(this.retryTimer);
    this.retryTimer = null;
    this.ws?.close();
  }
}
