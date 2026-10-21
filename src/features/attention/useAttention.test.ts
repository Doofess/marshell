import { afterEach, expect, it, vi } from "vitest";
import { mountHook } from "../../test/hooks";
import { useAttention } from "./useAttention";
import { setBadge } from "./attention";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));
vi.mock("./attention", async () => ({ ...await vi.importActual("./attention"), setBadge: vi.fn() }));
afterEach(() => vi.clearAllMocks());

it("sets only the badge count, once per change, and never a sound or a flash", async () => {
  const attention = await import("./attention");
  expect(Object.keys(attention)).not.toContain("alertNeedsYou");
  let entries: { id: string; tab: string }[] = [];
  const h = mountHook(() => useAttention(entries));
  expect(setBadge).toHaveBeenLastCalledWith(0);
  entries = [{ id: "a", tab: "a" }];
  h.rerender();
  expect(setBadge).toHaveBeenLastCalledWith(1);
  entries = [...entries, { id: "a2", tab: "a" }];
  h.rerender();
  expect(setBadge).toHaveBeenCalledTimes(2);
  h.unmount();
});
