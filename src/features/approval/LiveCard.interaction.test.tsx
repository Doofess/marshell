import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { elements, mountHook } from "../../test/hooks";
import { LiveCard } from "./LiveCard";
import { ApproveCard } from "./ApproveCard";
import { APPROVALS } from "./fixtures";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("window", Object.assign(new EventTarget(), { setTimeout, clearTimeout }));
  vi.stubGlobal("document", { activeElement: null });
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => setTimeout(() => cb(performance.now()), 10));
  vi.stubGlobal("cancelAnimationFrame", clearTimeout);
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

function card() {
  const inside = {};
  const node = { contains: (target: unknown) => target === inside || target === node };
  let request = { ...APPROVALS.safeBash, risk: { level: "risky" as const, reason: "Deletes files" } };
  const answer = vi.fn();
  const h = mountHook(() => {
    const wrapper = LiveCard({ request, onAnswer: answer, onJump: vi.fn(), onExpand: vi.fn(), onRestoreFocus: vi.fn() }) as any;
    if (wrapper.props.ref) wrapper.props.ref.current = node;
    const child = elements(wrapper).find((e) => e.type === ApproveCard)!;
    return { wrapper, view: ApproveCard(child.props) };
  });
  const advance = (ms: number) => { vi.advanceTimersByTime(ms); h.flush(); };
  const focus = (relatedTarget: unknown = null) => {
    Object.assign(document, { activeElement: inside });
    h.current.view.props.onFocus({ currentTarget: node, relatedTarget });
    h.current.wrapper.props.onFocus?.();
    h.flush();
  };
  const key = () => h.current.view.props.onKeyDown({ key: "y", preventDefault: vi.fn() });
  return { h, answer, inside, node, advance, focus, key,
    change: () => { request = { ...request, detail: { kind: "bash", command: "changed" } }; h.rerender(); },
  };
}

describe("risky hold safety", () => {
  it("cancels a keyboard hold when focus leaves the card", () => {
    const c = card(); c.focus(); c.advance(500); c.key(); c.advance(300);
    Object.assign(document, { activeElement: {} });
    c.h.current.wrapper.props.onBlur({ currentTarget: c.node, relatedTarget: {} });
    c.advance(600);
    expect(c.answer).not.toHaveBeenCalled();
    c.h.unmount();
  });
  it("cancels a hold on window blur", () => {
    const c = card(); c.focus(); c.advance(500); c.key(); c.advance(300);
    window.dispatchEvent(new Event("blur"));
    c.advance(600);
    expect(c.answer).not.toHaveBeenCalled();
    c.h.unmount();
  });
  it("cancels a hold when content changes and disarms the card", () => {
    const c = card(); c.focus(); c.advance(500); c.key(); c.advance(300);
    c.change(); c.advance(600);
    expect(c.answer).not.toHaveBeenCalled();
    c.h.unmount();
  });
  it("rechecks keyboard focus at completion even if focusout was missed", () => {
    const c = card(); c.focus(); c.advance(500); c.key();
    c.advance(590);
    Object.assign(document, { activeElement: {} });
    c.advance(10);
    expect(c.answer).not.toHaveBeenCalled();
    c.h.unmount();
  });
  it("allows once after a full armed hold with focus", () => {
    const c = card(); c.focus(); c.advance(500); c.key();
    c.advance(590); expect(c.answer).not.toHaveBeenCalled();
    c.advance(10); expect(c.answer).toHaveBeenCalledTimes(1);
    c.advance(700); expect(c.answer).toHaveBeenCalledTimes(1);
    c.h.unmount();
  });
});

describe("focus arming", () => {
  it("does not rearm when focus moves between controls inside the card", () => {
    const c = card(); c.focus(); c.advance(500);
    c.h.current.wrapper.props.onBlur({ currentTarget: c.node, relatedTarget: c.inside });
    c.focus(c.inside);
    expect(c.h.current.view.props["data-armed"]).toBe(true);
    c.h.unmount();
  });
  it("keeps the 500 ms guard on outside focus and on content changes", () => {
    const c = card(); c.advance(500); c.focus();
    c.advance(499); expect(c.h.current.view.props["data-armed"]).toBe(false);
    c.advance(1); expect(c.h.current.view.props["data-armed"]).toBe(true);
    c.change();
    c.advance(499); expect(c.h.current.view.props["data-armed"]).toBe(false);
    c.advance(1); expect(c.h.current.view.props["data-armed"]).toBe(true);
    c.h.unmount();
  });
});
