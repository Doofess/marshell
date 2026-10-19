import { TOKEN_SUBPROTOCOL_PREFIX, WS_SUBPROTOCOL } from "../../generated/constants";
import type { Endpoint } from "../../generated/Endpoint";
import type { PermissionView } from "../../generated/PermissionView";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";

const RETRY_BASE_MS = 250;
const RETRY_MAX_MS = 5000;

/**
 * The core's push channel. Every `sessions` frame is the whole list, so callers replace their state
 * on each one; so is every `permissions` frame (the requests being held). Reconnects with backoff; the core sends a fresh snapshot on every connect.
 */
export class EventSocket {
  private ws: WebSocket | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private retries = 0;
  private done = false;

  constructor(
    private readonly endpoint: Endpoint,
    private readonly onSnapshot: (s: SessionsSnapshot) => void,
    private readonly onPermissions?: (p: PermissionView[]) => void,
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
      if (this.done) return; // a closing socket can still deliver a frame
      const event = parseEvent(ev.data);
      if (event?.type === "sessions") this.onSnapshot(event.data);
      else if (event?.type === "permissions") this.onPermissions?.(event.data);
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

export type CoreEvent = { type: "sessions"; data: SessionsSnapshot } | { type: "permissions"; data: PermissionView[] };

/** The event inside a frame, or null for anything else. */
export function parseEvent(data: unknown): CoreEvent | null {
  if (typeof data !== "string") return null;
  try {
    const msg = JSON.parse(data) as { type?: unknown; data?: unknown };
    if (msg.type === "sessions" && msg.data) return { type: "sessions", data: msg.data as SessionsSnapshot };
    if (msg.type === "permissions" && Array.isArray(msg.data)) return { type: "permissions", data: msg.data as PermissionView[] };
    return null;
  } catch {
    return null;
  }
}
