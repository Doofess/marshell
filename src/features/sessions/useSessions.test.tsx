import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { SessionsSnapshot } from "../../generated/SessionsSnapshot";

const endpoints: Endpoint[] = [];
const sockets: { endpoint: Endpoint; onSnapshot: (s: SessionsSnapshot) => void; close: ReturnType<typeof vi.fn> }[] = [];
let coreCb: ((e: { kind: "restarted"; generation: number } | { kind: "failed" }) => void) | null = null;
const stopListening = vi.fn();
const resetEndpoint = vi.fn();

vi.mock("../../lib/api/client", () => ({
  getEndpoint: vi.fn(async () => {
    const ep: Endpoint = { port: 1000 + endpoints.length, token: "t", platform: "windows" };
    endpoints.push(ep);
    return ep;
  }),
  resetEndpoint: () => resetEndpoint(),
}));
vi.mock("../../lib/api/profiles", () => ({
  profiles: {
    get: vi.fn(async () => ({
      default_id: "pwsh",
      profiles: [{ id: "pwsh", name: "PowerShell", command: ["pwsh"], family: "power_shell", cwd: null, env: [], shell_integration: true, detected: true }],
    })),
  },
}));
vi.mock("../../lib/api/events", () => ({
  EventSocket: class {
    close = vi.fn();
    constructor(endpoint: Endpoint, onSnapshot: (s: SessionsSnapshot) => void) {
      sockets.push({ endpoint, onSnapshot, close: this.close });
    }
  },
}));
vi.mock("../../lib/api/coreEvents", () => ({
  onCoreEvent: vi.fn(async (cb: typeof coreCb) => {
    coreCb = cb;
    return stopListening;
  }),
}));

const { SessionsStore } = await import("./useSessions");
const { profiles: profilesApi } = await import("../../lib/api/profiles");

const info = (n: number, patch: Partial<SessionInfo> = {}): SessionInfo => ({
  tab_id: `t${n}`,
  number: n,
  name: `Shell ${n}`,
  agent: null,
  profile_id: "pwsh",
  cwd: "C:\\dev\\app",
  project: "C:\\dev\\app",
  mode: null,
  state: { kind: "running" },
  command: { kind: "unknown" },
  paused: false,
  ...patch,
});
const snap = (open: number[], closed: SessionInfo[] = []): SessionsSnapshot => ({ sessions: open.map((n) => info(n)), closed });

async function started() {
  const store = new SessionsStore();
  const stop = store.start();
  await vi.waitFor(() => expect(sockets.length).toBe(1));
  return { store, stop, socket: () => sockets[sockets.length - 1]! };
}

describe("SessionsStore", () => {
  beforeEach(() => {
    endpoints.length = 0;
    sockets.length = 0;
    coreCb = null;
    vi.clearAllMocks();
  });

  it("connects with the core's endpoint and loads the profiles", async () => {
    const { store, socket } = await started();
    expect(socket().endpoint).toEqual(endpoints[0]);
    expect(store.getState().endpoint).toEqual(endpoints[0]);
    expect(store.getState().profiles.map((p) => p.id)).toEqual(["pwsh"]);
    // An empty list is only "empty" once the core has sent one.
    expect(store.getState().loaded).toBe(false);
    socket().onSnapshot(snap([]));
    expect(store.getState().loaded).toBe(true);
  });

  it("selects the new session", async () => {
    const { store, socket } = await started();
    socket().onSnapshot(snap([]));
    socket().onSnapshot(snap([1]));
    expect(store.getState().selectedId).toBe("t1");
    socket().onSnapshot(snap([1, 2]));
    expect(store.getState().selectedId).toBe("t2");
    // A later snapshot of the same create (the state changed) does not move the selection.
    store.select("t1");
    socket().onSnapshot({ sessions: [info(1), info(2, { name: "renamed" })], closed: [] });
    expect(store.getState().selectedId).toBe("t1");
  });

  it("selection moves to the neighbour when the selected session closes", async () => {
    const { store, socket } = await started();
    socket().onSnapshot(snap([1, 2, 3]));
    store.select("t2");
    socket().onSnapshot(snap([1, 3]));
    expect(store.getState().selectedId).toBe("t3");
    // At the end, the previous one.
    socket().onSnapshot(snap([1]));
    expect(store.getState().selectedId).toBe("t1");
    socket().onSnapshot(snap([]));
    expect(store.getState().selectedId).toBeUndefined();
  });

  it("ignores a select for a session that does not exist", async () => {
    const { store, socket } = await started();
    socket().onSnapshot(snap([1]));
    store.select("nope");
    expect(store.getState().selectedId).toBe("t1");
  });

  it("keeps the closed list apart from the open sessions", async () => {
    const { store, socket } = await started();
    socket().onSnapshot(snap([1], [info(2, { state: { kind: "exited", code: 1 } })]));
    expect(store.getState().infos.map((i) => i.tab_id)).toEqual(["t1"]);
    expect(store.getState().closed.map((i) => i.tab_id)).toEqual(["t2"]);
  });

  it("core restart clears the list and reconnects to the new core", async () => {
    const { store, socket } = await started();
    socket().onSnapshot(snap([1, 2]));
    const old = socket();
    coreCb!({ kind: "restarted", generation: 2 });
    expect(resetEndpoint).toHaveBeenCalled();
    expect(old.close).toHaveBeenCalled();
    expect(store.getState().infos).toEqual([]);
    expect(store.getState().selectedId).toBeUndefined();
    expect(store.getState().loaded).toBe(false);
    await vi.waitFor(() => expect(sockets.length).toBe(2));
    expect(sockets[1]!.endpoint).toEqual(endpoints[1]);
    // A late frame from the old socket is ignored.
    old.onSnapshot(snap([1, 2]));
    expect(store.getState().infos).toEqual([]);
    sockets[1]!.onSnapshot(snap([1]));
    expect(store.getState().selectedId).toBe("t1");
  });

  it("counts core restarts, so the window can open the launcher before the first snapshot", async () => {
    const { store, socket } = await started();
    expect(store.getState().restarts).toBe(0);
    socket().onSnapshot(snap([1]));
    coreCb!({ kind: "restarted", generation: 2 });
    expect(store.getState().restarts).toBe(1);
    expect(store.getState().loaded).toBe(false);
    await vi.waitFor(() => expect(sockets.length).toBe(2));
    sockets[1]!.onSnapshot(snap([]));
    coreCb!({ kind: "restarted", generation: 3 });
    expect(store.getState().restarts).toBe(2);
  });

  it("reloading the profiles picks up a saved change, so new sessions start with the saved theme", async () => {
    const { store } = await started();
    expect(store.getState().profiles[0]!.terminal_theme).toBeUndefined();
    const saved = { default_id: "pwsh", profiles: [{ ...(await profilesApi.get(endpoints[0]!)).profiles[0]!, terminal_theme: "dark" }] };
    vi.mocked(profilesApi.get).mockResolvedValueOnce(saved);
    await store.reloadProfiles();
    expect(store.getState().profiles[0]!.terminal_theme).toBe("dark");
    // A reload that fails keeps the list it had.
    vi.mocked(profilesApi.get).mockRejectedValueOnce(new Error("down"));
    await store.reloadProfiles();
    expect(store.getState().profiles[0]!.terminal_theme).toBe("dark");
  });

  it("a failed core stops the socket and says so", async () => {
    const { store, socket } = await started();
    coreCb!({ kind: "failed" });
    expect(socket().close).toHaveBeenCalled();
    expect(store.getState().error).toMatch(/could not restart/);
  });

  it("stop closes the socket and stops listening", async () => {
    const { socket, stop } = await started();
    await vi.waitFor(() => expect(coreCb).not.toBeNull());
    stop();
    expect(socket().close).toHaveBeenCalled();
    await vi.waitFor(() => expect(stopListening).toHaveBeenCalled());
  });
});
