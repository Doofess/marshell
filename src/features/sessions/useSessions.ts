import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";
import { getEndpoint, resetEndpoint } from "../../lib/api/client";
import { onCoreEvent } from "../../lib/api/coreEvents";
import { EventSocket } from "../../lib/api/events";
import { profiles as profilesApi } from "../../lib/api/profiles";
import type { RowModel } from "../sidebar/types";
import type { HookInstall } from "../../generated/HookInstall";
import { ageTick, toRow } from "./toRow";

export type SessionsState = {
  endpoint: Endpoint | null;
  profiles: Profile[];
  /** Open sessions in tab order. */
  infos: SessionInfo[];
  /** Recently closed sessions. Never shown as live rows: an entry can carry a stale state or number. */
  closed: SessionInfo[];
  selectedId: string | undefined;
  /** True once the core has sent the list on this connection, so an empty list really is empty. */
  loaded: boolean;
  /** How many times the core restarted. The new core seeds the saved tabs as Ended, so a restart does not by itself open the launcher. */
  restarts: number;
  error: string | null;
};

const EMPTY: SessionsState = { endpoint: null, profiles: [], infos: [], closed: [], selectedId: undefined, loaded: false, restarts: 0, error: null };
const FAILED_TEXT = "Marshell's core stopped and could not restart. Restart Marshell to continue.";

/** Which session is selected after the list changed: the newest one, else the same one, else its neighbour. */
function nextSelection(prev: SessionInfo[], next: SessionInfo[], selectedId: string | undefined, firstSnapshot: boolean): string | undefined {
  const known = new Set(prev.map((s) => s.tab_id));
  const added = next.filter((s) => !known.has(s.tab_id));
  // The very first list after connecting is not a "create": start on its first session.
  if (added.length > 0) return (firstSnapshot ? added[0] : added[added.length - 1])!.tab_id;
  const present = new Set(next.map((s) => s.tab_id));
  if (selectedId !== undefined && present.has(selectedId)) return selectedId;
  // The selected one closed: the next one after it in the old order, else the one before it.
  const at = prev.findIndex((s) => s.tab_id === selectedId);
  if (at >= 0) {
    const after = prev.slice(at + 1).find((s) => present.has(s.tab_id));
    const before = prev.slice(0, at).reverse().find((s) => present.has(s.tab_id));
    return (after ?? before)?.tab_id;
  }
  return next[0]?.tab_id;
}

/**
 * The core's live session list. It connects to the push channel, replaces the list on every frame, and starts over on a core
 * restart (the old sessions died with the old core). Plain class so it can be tested without a DOM; `useSessions` wraps it.
 */
export class SessionsStore {
  private state: SessionsState = EMPTY;
  private listeners = new Set<() => void>();
  /** Bumped on every connect, restart and stop, so a late answer or frame from an older connection is dropped. */
  private generation = 0;
  private socket: EventSocket | null = null;
  private gotSnapshot = false;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getState = (): SessionsState => this.state;

  private set(patch: Partial<SessionsState>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l());
  }

  select = (id: string): void => {
    if (this.state.infos.some((s) => s.tab_id === id)) this.set({ selectedId: id });
  };

  /** Reads the profile list again, after the profiles page saved, so new sessions start with what was saved. A failed read keeps the old list. */
  reloadProfiles = async (): Promise<void> => {
    const { endpoint } = this.state;
    const gen = this.generation;
    if (!endpoint) return;
    const doc = await profilesApi.get(endpoint).catch(() => null);
    if (doc && gen === this.generation) this.set({ profiles: doc.profiles });
  };

  /** Connects, and listens for core restarts. Returns the function that stops both. */
  start(): () => void {
    let stopped = false;
    let unlisten: (() => void) | null = null;
    onCoreEvent((e) => {
      if (stopped) return;
      if (e.kind === "restarted") {
        resetEndpoint();
        this.disconnect();
        this.set({ ...EMPTY, restarts: this.state.restarts + 1 });
        void this.connect();
      } else {
        this.disconnect();
        this.set({ error: FAILED_TEXT });
      }
    })
      .then((stop) => (stopped ? stop() : (unlisten = stop)))
      .catch(() => {}); // Outside Tauri (tests, a plain browser) there are no core events.
    void this.connect();
    return () => {
      stopped = true;
      unlisten?.();
      this.disconnect();
    };
  }

  private disconnect(): void {
    this.generation++;
    this.socket?.close();
    this.socket = null;
    this.gotSnapshot = false;
  }

  private async connect(): Promise<void> {
    const gen = ++this.generation;
    try {
      const endpoint = await getEndpoint();
      // A profile list that won't load only costs the shell marks (rows fall back to the generic one), so it is not an error.
      const doc = await profilesApi.get(endpoint).catch(() => null);
      if (gen !== this.generation) return;
      this.socket = new EventSocket(endpoint, (snapshot) => gen === this.generation && this.apply(snapshot));
      this.set({ endpoint, profiles: doc?.profiles ?? [], error: null });
    } catch (e) {
      if (gen === this.generation) this.set({ error: `Couldn't reach the core: ${String(e)}` });
    }
  }

  private apply(snapshot: SessionsSnapshot): void {
    const first = !this.gotSnapshot;
    this.gotSnapshot = true;
    const selectedId = nextSelection(this.state.infos, snapshot.sessions, this.state.selectedId, first);
    this.set({ infos: snapshot.sessions, closed: snapshot.closed, selectedId, loaded: true });
  }
}

export function useSessions(setup?: { claude?: HookInstall }): {
  endpoint: Endpoint | null;
  profiles: Profile[];
  rows: RowModel[];
  infos: SessionInfo[];
  closed: SessionInfo[];
  selectedId: string | undefined;
  loaded: boolean;
  restarts: number;
  select: (id: string) => void;
  reloadProfiles: () => Promise<void>;
  error: string | null;
} {
  const store = useMemo(() => new SessionsStore(), []);
  useEffect(() => store.start(), [store]);
  const state = useSyncExternalStore(store.subscribe, store.getState);
  // The clock the rows' ages count from. It moves only while some row shows an age, as often as that age needs.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const tick = ageTick(state.infos, Date.now());
    if (tick === undefined) return;
    const id = setTimeout(() => setNow(Date.now()), tick);
    return () => clearTimeout(id);
  }, [state.infos, now]);
  const rows = useMemo(() => state.infos.map((i) => toRow(i, state.profiles, setup, Math.max(now, Date.now()))), [state.infos, state.profiles, setup, now]);
  return { ...state, rows, select: store.select, reloadProfiles: store.reloadProfiles };
}
