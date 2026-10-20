import { afterEach, expect, it, vi } from "vitest";
import { mountHook } from "../../test/hooks";
import { useAttention } from "./useAttention";
import { alertNeedsYou, setBadge } from "./attention";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));
vi.mock("./attention", async () => ({ ...await vi.importActual("./attention"), alertNeedsYou: vi.fn(), setBadge: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); vi.clearAllMocks(); });

it("checks document focus at each step after a top-window blur into an iframe", () => {
  const win = new EventTarget();
  const hasFocus = vi.fn(() => true);
  vi.stubGlobal("window", win);
  vi.stubGlobal("document", { hasFocus });
  let entries: { id: string; tab: string }[] = [];
  const h = mountHook(() => useAttention(entries, new Set(), () => false));
  win.dispatchEvent(new Event("blur"));
  entries = [{ id: "a", tab: "a" }];
  h.rerender();
  expect(setBadge).toHaveBeenLastCalledWith(1);
  expect(alertNeedsYou).not.toHaveBeenCalled();
  hasFocus.mockReturnValue(false);
  entries = [...entries, { id: "b", tab: "b" }];
  h.rerender();
  expect(alertNeedsYou).toHaveBeenCalledExactlyOnceWith(true);
  h.unmount();
});
