// Spike 6q-0 (never merged): the browser pane in the preview slot. Rust draws the page as a child webview over the
// placeholder; this side only measures the placeholder and drives the child through the spike commands.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type CSSProperties, type FocusEvent } from "react";
import "./BrowserSpike.css";
import { spikeLog, type BrowserSpikeModel } from "./useBrowserSpike";

// The commands run one after another, in the order they were asked for: StrictMode mounts twice in dev, and a hide must
// never overtake the show that follows it.
let queue: Promise<unknown> = Promise.resolve();
export function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const next = queue.then(() => invoke<T>(cmd, args));
  queue = next.catch(() => {});
  return next;
}

function measure(el: HTMLElement) {
  const r = el.getBoundingClientRect();
  return { x: r.x, y: r.y, width: r.width, height: r.height, winWidth: window.innerWidth, winHeight: window.innerHeight };
}

const TABBABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** The app's tab order, as Tab walks it: shown, not inert, not taken out with tabindex -1. */
function tabbables(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(TABBABLE)).filter((e) => e.tabIndex >= 0 && e.getClientRects().length > 0 && !e.closest("[inert]"));
}

function describe(e: Element | null): string | null {
  if (!e) return null;
  const label = e.getAttribute("aria-label");
  return `${e.tagName.toLowerCase()}${label ? ` "${label}"` : ""}`;
}

/** A press on the title bar's drag area moves the window; it is not the user picking something in the app. */
function onDragArea(t: Element): boolean {
  const own = t.getAttribute("data-tauri-drag-region");
  if (own !== null && own !== "false") return true;
  return t.closest('[data-tauri-drag-region="deep"]') !== null && t.closest('button, a, input, [role="button"]') === null;
}

// Whether the page holds the keyboard. Alt+Tab back hands focus to the app's webview (wry, S12 finding 4); the pane gives
// it back to the page unless the user clicked in the app or pressed an app key since.
let pageOwns = false;

type Props = { model: BrowserSpikeModel; frozen: boolean };

export function BrowserSpike({ model, frozen }: Props) {
  const page = useRef<HTMLDivElement>(null);
  const [address, setAddress] = useState(model.request?.url ?? "");
  const [status, setStatus] = useState("");
  const { created, request } = model;
  void frozen; // Task 5

  // Opens the page the first time, goes to a new address after; a pane shown again shows the page it already has.
  useEffect(() => {
    const el = page.current;
    if (!el || !request) return;
    setAddress(request.url);
    const first = !created.current;
    created.current = true;
    call("spike_browser_open", { url: request.url, rect: measure(el) }).catch((e) => {
      if (first) created.current = false;
      setStatus(String(e));
    });
  }, [request, created]);

  useEffect(() => {
    const el = page.current;
    if (!el) return;
    if (created.current) {
      void call("spike_browser_visible", { visible: true }).catch(() => {});
      void call("spike_browser_rect", { rect: measure(el) }).catch(() => {});
    }
    const report = () => {
      if (created.current) void call("spike_browser_rect", { rect: measure(el) }).catch(() => {});
    };
    const ro = new ResizeObserver(report);
    ro.observe(el);
    window.addEventListener("resize", report);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", report);
      if (created.current) void call("spike_browser_visible", { visible: false }).catch(() => {});
    };
  }, [created]);

  // Tab or Shift+Tab ran off an end of the page: focus goes to the app element on that side, as it would past any element.
  useEffect(() => {
    const off = listen<{ direction: "next" | "previous" }>("spike://focus-out", ({ payload }) => {
      const el = page.current;
      if (!el) return;
      const all = tabbables();
      const i = all.indexOf(el);
      const target = payload.direction === "next" ? (all[i + 1] ?? all[0]) : (all[i - 1] ?? all[all.length - 1]);
      pageOwns = false;
      target?.focus();
      spikeLog("focus-out-landed", { direction: payload.direction, on: describe(document.activeElement) });
    });
    return () => void off.then((f) => f());
  }, []);

  useEffect(() => {
    const offPage = listen("spike://page-focus", () => {
      pageOwns = true;
    });
    const offKey = listen("spike://key", () => {
      pageOwns = false;
    });
    let pointerAt = 0;
    const onPointer = (e: PointerEvent) => {
      if (e.target instanceof Element && onDragArea(e.target)) return;
      pointerAt = performance.now();
      pageOwns = false;
    };
    const onWindowFocus = () => {
      if (!pageOwns || !created.current) return;
      const at = performance.now();
      setTimeout(() => {
        if (!pageOwns || pointerAt > at || !document.hasFocus()) return;
        spikeLog("page-enter", { reason: "programmatic", from: "window-focus" });
        void call("spike_browser_focus", { reason: "programmatic" }).catch(() => {});
      }, 100);
    };
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("focus", onWindowFocus);
    return () => {
      void offPage.then((f) => f());
      void offKey.then((f) => f());
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("focus", onWindowFocus);
    };
  }, [created]);

  useEffect(() => {
    const off = listen<{ url: string }>("spike://refused", (e) => {
      setStatus(`Refused ${e.payload.url}`);
      spikeLog("refused-shown", { url: e.payload.url });
    });
    return () => void off.then((f) => f());
  }, []);

  // Tab reaching the placeholder goes into the page: from the element before it to the page's first element, from the one
  // after it to its last. Focus arriving with no element before it is the window coming back, handled above.
  const onPageFocus = (e: FocusEvent<HTMLDivElement>) => {
    const from = e.relatedTarget;
    if (!created.current || !(from instanceof Element)) return;
    const reason = e.currentTarget.compareDocumentPosition(from) & Node.DOCUMENT_POSITION_FOLLOWING ? "previous" : "next";
    spikeLog("page-enter", { reason, from: describe(from) });
    void call("spike_browser_focus", { reason }).catch(() => {});
  };

  const go = () => {
    setStatus("");
    call("spike_browser_open", { url: address, rect: page.current ? measure(page.current) : null }).catch((e) => setStatus(String(e)));
  };

  return (
    <section className="spike-browser" aria-label="Browser (spike)" style={{ "--spike-inset": `${model.inset}px` } as CSSProperties}>
      <div className="spike-browser__bar">
        <button type="button" disabled>
          Back
        </button>
        <button type="button" disabled>
          Forward
        </button>
        <button type="button" disabled>
          Reload
        </button>
        <input aria-label="Address" value={address} onChange={(e) => setAddress(e.target.value)} onKeyDown={(e) => e.key === "Enter" && go()} />
      </div>
      <p className="spike-browser__status">
        {status}
      </p>
      <div ref={page} className="spike-browser__page" tabIndex={0} aria-label="Web page" onFocus={onPageFocus} />
    </section>
  );
}
