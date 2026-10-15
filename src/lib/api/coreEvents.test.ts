import { beforeEach, describe, expect, it, vi } from "vitest";

const handlers = new Map<string, (e: { payload: unknown }) => void>();
const unlisteners = { "core-restarted": vi.fn(), "core-failed": vi.fn() };

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: keyof typeof unlisteners, handler: (e: { payload: unknown }) => void) => {
    handlers.set(name, handler);
    return unlisteners[name];
  }),
}));

const { onCoreEvent } = await import("./coreEvents");

describe("onCoreEvent", () => {
  beforeEach(() => {
    handlers.clear();
    vi.clearAllMocks();
  });

  it("listens for both core events", async () => {
    await onCoreEvent(() => {});
    expect([...handlers.keys()].sort()).toEqual(["core-failed", "core-restarted"]);
  });

  it("maps a restart with its generation, and a failure", async () => {
    const cb = vi.fn();
    await onCoreEvent(cb);
    handlers.get("core-restarted")!({ payload: { generation: 2 } });
    handlers.get("core-failed")!({ payload: null });
    expect(cb.mock.calls).toEqual([[{ kind: "restarted", generation: 2 }], [{ kind: "failed" }]]);
  });

  it("returns one function that stops both listeners", async () => {
    const stop = await onCoreEvent(() => {});
    stop();
    expect(unlisteners["core-restarted"]).toHaveBeenCalledOnce();
    expect(unlisteners["core-failed"]).toHaveBeenCalledOnce();
  });
});
