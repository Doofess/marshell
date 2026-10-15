import { TOKEN_SUBPROTOCOL_PREFIX, WS_SUBPROTOCOL } from "../../generated/constants";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";

const RETRY_BASE_MS = 250;
const RETRY_MAX_MS = 5000;

/**
 * The core's push channel. Every `sessions` frame is the whole list, so callers replace their state
 * on each one. Reconnects with backoff; the core sends a fresh snapshot on every connect.
 */
export class EventSocket {
  private ws: WebSocket | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private retries = 0;
  private done = false;

  constructor(
    private readonly endpoint: Endpoint,
    private readonly onSnapshot: (s: SessionsSnapshot) => void,
  ) {
    this.connect();
  }

  private connect(): void {
    if (this.done) return;
    const url = `ws://127.0.0.1:${this.endpoint.port}/v1/events`;
    const ws = new WebSocket(url, [WS_SUBPROTOCOL, TOKEN_SUBPROTOCOL_PREFIX + this.endpoint.token]);
    ws.onopen = () => {
      this.retries = 0;
    };
    ws.onmessage = (ev) => {
      const snapshot = parseSnapshot(ev.data);
      if (snapshot) this.onSnapshot(snapshot);
    };
    ws.onclose = () => {
      this.ws = null;
      if (this.done) return;
      const delay = Math.min(RETRY_MAX_MS, RETRY_BASE_MS * 2 ** this.retries++);
      this.retryTimer = setTimeout(() => {
        this.retryTimer = null;
        this.connect();
      }, delay);
    };
    this.ws = ws;
  }

  close(): void {
    this.done = true;
    if (this.retryTimer !== null) clearTimeout(this.retryTimer);
    this.retryTimer = null;
    this.ws?.close();
  }
}

/** The snapshot inside a `sessions` frame, or null for anything else. */
function parseSnapshot(data: unknown): SessionsSnapshot | null {
  if (typeof data !== "string") return null;
  try {
    const msg = JSON.parse(data) as { type?: unknown; data?: SessionsSnapshot };
    if (msg.type !== "sessions" || !msg.data) return null;
    return msg.data;
  } catch {
    return null;
  }
}
