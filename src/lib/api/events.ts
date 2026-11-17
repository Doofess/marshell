import { TOKEN_SUBPROTOCOL_PREFIX, WS_SUBPROTOCOL } from "../../generated/constants";
import type { DoctorSummary } from "../../generated/DoctorSummary";
import type { Endpoint } from "../../generated/Endpoint";
import type { PaneOpen } from "../../generated/PaneOpen";
import type { PermissionView } from "../../generated/PermissionView";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";

const RETRY_BASE_MS = 250;
const RETRY_MAX_MS = 5000;

/**
 * The core's push channel. Every `sessions` frame is the whole list, so callers replace their state
 * on each one; so is every `permissions` frame (the requests being held) and every `opens` frame (the `marshell open` requests still waiting). Reconnects with backoff; the core sends a fresh snapshot on every connect.
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
    private readonly onDoctor?: (d: DoctorSummary) => void,
    private readonly onOpens?: (opens: PaneOpen[]) => void,
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
      else if (event?.type === "doctor") this.onDoctor?.(event.data);
      else if (event?.type === "opens") this.onOpens?.(event.data);
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

export type CoreEvent = { type: "sessions"; data: SessionsSnapshot } | { type: "permissions"; data: PermissionView[] } | { type: "doctor"; data: DoctorSummary } | { type: "opens"; data: PaneOpen[] };

/** A list of open requests, each with the fields the app reads. */
function isOpens(data: unknown): data is PaneOpen[] {
  return (
    Array.isArray(data) &&
    data.every((o) => typeof o === "object" && o !== null && typeof o.id === "number" && typeof o.tab_id === "string" && typeof o.kind === "string" && typeof o.target === "string" && typeof o.by === "string")
  );
}

/** The event inside a frame, or null for anything else. */
export function parseEvent(data: unknown): CoreEvent | null {
  if (typeof data !== "string") return null;
  try {
    const msg = JSON.parse(data) as { type?: unknown; data?: unknown };
    if (msg.type === "sessions" && msg.data) return { type: "sessions", data: msg.data as SessionsSnapshot };
    if (msg.type === "permissions" && Array.isArray(msg.data)) return { type: "permissions", data: msg.data as PermissionView[] };
    if (msg.type === "doctor" && msg.data) return { type: "doctor", data: msg.data as DoctorSummary };
    if (msg.type === "opens" && isOpens(msg.data)) return { type: "opens", data: msg.data };
    return null;
  } catch {
    return null;
  }
}
