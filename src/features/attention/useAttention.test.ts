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
  let count = 0;
  const h = mountHook(() => useAttention(count));
  expect(setBadge).toHaveBeenLastCalledWith(0);
  count = 1;
  h.rerender();
  expect(setBadge).toHaveBeenLastCalledWith(1);
  h.rerender();
  expect(setBadge).toHaveBeenCalledTimes(2);
  h.unmount();
});
