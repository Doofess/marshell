import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { mountHook } from "../../test/hooks";
import { useViewReport } from "./useViewReport";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));

const ep = (port: number) => ({ port, token: "t", platform: "windows" }) as Endpoint;
let win: EventTarget;
let focused: boolean;
const send = vi.fn(async (_ep: Endpoint, _report: { window_focused: boolean; visible_tabs: string[] }) => {});

beforeEach(() => {
  vi.useFakeTimers();
  win = new EventTarget();
  focused = true;
  vi.stubGlobal("window", win);
  vi.stubGlobal("document", { hasFocus: () => focused });
  send.mockClear();
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

it("reports once on mount", () => {
  const h = mountHook(() => useViewReport(ep(1), ["a"], send));
  expect(send).toHaveBeenCalledExactlyOnceWith(ep(1), { window_focused: true, visible_tabs: ["a"] });
  h.unmount();
});

it("reports a blur and a focus, at most once per 200 ms", () => {
  const h = mountHook(() => useViewReport(ep(1), ["a"], send));
  send.mockClear();
  focused = false;
  win.dispatchEvent(new Event("blur"));
  expect(send).not.toHaveBeenCalled();
  focused = true;
  win.dispatchEvent(new Event("focus"));
  focused = false;
  win.dispatchEvent(new Event("blur"));
  vi.advanceTimersByTime(250);
  expect(send).toHaveBeenCalledExactlyOnceWith(ep(1), { window_focused: false, visible_tabs: ["a"] });
  h.unmount();
});

it("reports a pane change", () => {
  let tabs = ["a"];
  const h = mountHook(() => useViewReport(ep(1), tabs, send));
  vi.advanceTimersByTime(250);
  send.mockClear();
  tabs = ["a", "b"];
  h.rerender();
  expect(send).toHaveBeenCalledExactlyOnceWith(ep(1), { window_focused: true, visible_tabs: ["a", "b"] });
  h.unmount();
});

it("never sends the same value twice", () => {
  const h = mountHook(() => useViewReport(ep(1), ["a"], send));
  vi.advanceTimersByTime(250);
  send.mockClear();
  win.dispatchEvent(new Event("focus"));
  vi.advanceTimersByTime(250);
  win.dispatchEvent(new Event("blur"));
  vi.advanceTimersByTime(250);
  expect(send).not.toHaveBeenCalled();
  h.unmount();
});

it("sends again after a core restart, which hands over a new endpoint", () => {
  let endpoint = ep(1);
  const h = mountHook(() => useViewReport(endpoint, ["a"], send));
  vi.advanceTimersByTime(250);
  send.mockClear();
  endpoint = ep(2);
  h.rerender();
  vi.advanceTimersByTime(250);
  expect(send).toHaveBeenCalledExactlyOnceWith(ep(2), { window_focused: true, visible_tabs: ["a"] });
  h.unmount();
});

it("sends nothing without an endpoint and nothing after unmount", () => {
  mountHook(() => useViewReport(undefined, ["a"], send)).unmount();
  expect(send).not.toHaveBeenCalled();
  const h = mountHook(() => useViewReport(ep(1), ["a"], send));
  send.mockClear();
  h.unmount();
  focused = false;
  win.dispatchEvent(new Event("blur"));
  vi.advanceTimersByTime(500);
  expect(send).not.toHaveBeenCalled();
});

it("sends at most 16 tabs", () => {
  const many = Array.from({ length: 20 }, (_, i) => `t${i}`);
  const h = mountHook(() => useViewReport(ep(1), many, send));
  expect(send.mock.calls[0]![1].visible_tabs).toHaveLength(16);
  h.unmount();
});
