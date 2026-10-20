import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mountHook } from "../../test/hooks";
import { useCards, EXIT_MS, RECEIPT_MS, GONE_MESSAGE, UNREACHABLE_MESSAGE } from "./LiveCard";
import { APPROVALS } from "./fixtures";
import type { ApprovalRequest } from "./types";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
function setup() {
  let entries: ApprovalRequest[] = [APPROVALS.safeBash];
  const notice = vi.fn();
  const h = mountHook(() => useCards(entries, notice));
  return { h, notice, vanish: () => { entries = []; h.rerender(); } };
}
const settle = async () => { await Promise.resolve(); await Promise.resolve(); };

describe("useCards", () => {
  it("sends a double answer only once before and after a render", async () => {
    const { h } = setup();
    const call = vi.fn(() => new Promise<"ok">(() => {}));
    h.current.answer(APPROVALS.safeBash.id, "allowed", call);
    h.current.answer(APPROVALS.safeBash.id, "denied", call);
    expect(call).toHaveBeenCalledTimes(1);
    h.flush();
    h.current.answer(APPROVALS.safeBash.id, "denied", call);
    expect(call).toHaveBeenCalledTimes(1);
    h.unmount();
  });
  it("keeps a vanished card while sending, then shows its receipt and removes it", async () => {
    const { h, vanish } = setup();
    let resolve!: (value: "ok") => void;
    h.current.answer(APPROVALS.safeBash.id, "allowed", () => new Promise((done) => { resolve = done; }));
    h.flush(); vanish();
    expect(h.current.requests).toHaveLength(1);
    expect(h.current.requests[0]).toMatchObject({ busy: true, state: { phase: "pending" } });
    vi.advanceTimersByTime(5000); h.flush();
    expect(h.current.requests).toHaveLength(1);
    resolve("ok"); await settle(); h.flush();
    expect(h.current.requests[0]?.state).toEqual({ phase: "decided", verdict: "allowed" });
    vi.advanceTimersByTime(RECEIPT_MS); h.flush();
    expect(h.current.requests[0]?.leaving).toBe(true);
    vi.advanceTimersByTime(EXIT_MS); h.flush();
    expect(h.current.requests).toEqual([]);
    h.unmount();
  });
  it("lets a vanished idle card leave and removes it after the exit", () => {
    const { h, vanish } = setup();
    vanish();
    expect(h.current.requests[0]?.leaving).toBe(true);
    vi.advanceTimersByTime(EXIT_MS - 1); h.flush();
    expect(h.current.requests).toHaveLength(1);
    vi.advanceTimersByTime(1); h.flush();
    expect(h.current.requests).toEqual([]);
    h.unmount();
  });
  it.each([
    ["gone", GONE_MESSAGE],
    [{ conflict: "No rule to add" }, "No rule to add"],
    ["ok", "Allowed · npm test"],
  ] as const)("routes %j through the shared announcement callback once", async (result, text) => {
    const { h, notice } = setup();
    expect(notice).not.toHaveBeenCalled();
    h.current.answer(APPROVALS.safeBash.id, "allowed", async () => result);
    await settle(); h.flush(); h.rerender();
    expect(notice).toHaveBeenCalledExactlyOnceWith(text);
    h.unmount();
  });
  it("announces a failed answer through the shared callback", async () => {
    const { h, notice } = setup();
    h.current.answer(APPROVALS.safeBash.id, "allowed", async () => { throw new Error("offline"); });
    await settle(); h.flush();
    expect(notice).toHaveBeenCalledExactlyOnceWith(UNREACHABLE_MESSAGE);
    h.unmount();
  });
});
