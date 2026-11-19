import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { elements, mountHook } from "../../../test/hooks";
import { PdfSidebar } from "./PdfSidebar";

vi.mock("react", async () => ({ ...(await vi.importActual("react")), ...(await import("../../../test/hooks")).hooks }));

// "Show pages" goes away as the view changes, so it must hand the keyboard on, or it falls to the page body.
describe("PdfSidebar's empty outline", () => {
  const pagesTab = { focus: vi.fn() };
  beforeEach(() => {
    pagesTab.focus.mockClear();
    vi.stubGlobal("document", { getElementById: (id: string) => (id === "test-id-tab-pages" ? pagesTab : null) });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("show_pages_takes_the_keyboard_to_the_pages_view", () => {
    const onView = vi.fn();
    const h = mountHook(() =>
      PdfSidebar({ id: "side", name: "a.pdf", view: "outline", onView, outline: [], section: null, expanded: new Set(), onToggle: vi.fn(), onPick: vi.fn(), onLeave: vi.fn(), pages: null }),
    );
    const show = elements(h.current).find((e) => e.type === "button" && e.props.children === "Show pages")!;
    show.props.onClick();
    expect(onView).toHaveBeenCalledWith("pages", true);
  });

  // Removing the focused button fires focusout with nowhere to go, which lying over the pages closes the sidebar (live recheck, 2026-11-19).
  it("show_pages_moves_the_keyboard_to_the_pages_tab_before_the_button_goes", () => {
    const onView = vi.fn();
    const h = mountHook(() =>
      PdfSidebar({ id: "side", name: "a.pdf", view: "outline", onView, outline: [], section: null, expanded: new Set(), onToggle: vi.fn(), onPick: vi.fn(), onLeave: vi.fn(), pages: null }),
    );
    elements(h.current).find((e) => e.type === "button" && e.props.children === "Show pages")!.props.onClick();
    expect(pagesTab.focus).toHaveBeenCalledTimes(1);
    expect(pagesTab.focus.mock.invocationCallOrder[0]).toBeLessThan(onView.mock.invocationCallOrder[0]!);
  });

  it("a_tab_click_keeps_the_keyboard_on_the_tab", () => {
    const onView = vi.fn();
    const h = mountHook(() =>
      PdfSidebar({ id: "side", name: "a.pdf", view: "outline", onView, outline: [], section: null, expanded: new Set(), onToggle: vi.fn(), onPick: vi.fn(), onLeave: vi.fn(), pages: null }),
    );
    const tab = elements(h.current).find((e) => e.type === "button" && e.props.role === "tab" && e.props.children === "Pages")!;
    tab.props.onClick();
    expect(onView).toHaveBeenCalledWith("pages");
  });
});
