import { listen } from "@tauri-apps/api/event";

export type CoreEvent = { kind: "restarted"; generation: number } | { kind: "failed" };

/** Calls `cb` when the core restarts (its sessions ended) or gives up. Resolves to one function that stops listening. */
export async function onCoreEvent(cb: (e: CoreEvent) => void): Promise<() => void> {
  const stops = await Promise.all([
    listen<{ generation: number }>("core-restarted", (e) => cb({ kind: "restarted", generation: e.payload.generation })),
    listen("core-failed", () => cb({ kind: "failed" })),
  ]);
  return () => stops.forEach((stop) => stop());
}
