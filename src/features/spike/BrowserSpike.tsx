// Spike 6q-0 (never merged): the browser pane in the preview slot. Rust draws the page as a child webview over the
// placeholder; this side only measures the placeholder and drives the child through the spike commands.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type CSSProperties, type FocusEvent } from "react";
import "./BrowserSpike.css";
import { paneRule, type RuleState } from "./paneRule";
import { paintSync, spikeLog, type BrowserSpikeModel } from "./useBrowserSpike";

// The commands run one after another, in the order they were asked for: StrictMode mounts twice in dev, and a hide must
// never overtake the show that follows it.
let queue: Promise<unknown> = Promise.resolve();
// Commands asked for and not finished yet; the lag check logs it with each report.
let pending = 0;
/** `args` as a function is read when the command's turn comes, not when it is asked for. */
export function call<T>(cmd: string, args?: Record<string, unknown> | (() => Record<string, unknown>)): Promise<T> {
  pending += 1;
  const next = queue
    .then(() => invoke<T>(cmd, typeof args === "function" ? args() : args))
    .finally(() => {
      pending -= 1;
    });
  queue = next.catch(() => {});
  return next;
}

/** A bounds report, stamped for the lag check: when it was measured (epoch ms) and how many commands were not done yet. */
const rectArgs = (el: HTMLElement) => ({ rect: measure(el), sentAt: performance.timeOrigin + performance.now(), queued: pending });

// The page's bounds, latest only. While the user drags the window's edge, Windows' sizing loop makes each command's round
// trip longer than a frame, and one report queued per frame fell further and further behind: up to 121 waiting, the page
// 0.9 s late (S12 finding 20). So at most one report waits, and it measures the placeholder when its turn comes.
let rectWaiting = false;
// The window's size at the last report. A report at the same size is a change inside the window, where no window edge
// moves: with `paintSync.frames` above 0 the page waits for the app to paint that frame, so it moves with the app's own
// picture (the URL bar, the divider) instead of a frame ahead of it.
let lastSize = "";
let held: ReturnType<typeof rectArgs> | null = null;
let holding = false;
function afterFrames(n: number): Promise<void> {
  return new Promise((done) => {
    const step = (left: number) => requestAnimationFrame(() => (left > 1 ? step(left - 1) : setTimeout(done, 0)));
    step(n);
  });
}
function reportRect(el: HTMLElement): void {
  const args = rectArgs(el);
  const size = `${args.rect.winWidth}x${args.rect.winHeight}`;
  const inWindow = paintSync.frames > 0 && size === lastSize;
  lastSize = size;
  if (inWindow) {
    // The latest measurement this frame; one send per frame, after the paint.
    held = args;
    if (holding) return;
    holding = true;
    void afterFrames(paintSync.frames).then(() => {
      holding = false;
      const a = held;
      held = null;
      if (a) void call("spike_browser_rect", a).catch(() => {});
    });
    return;
  }
  // The window resized: a held report is for the old size, and this one goes now.
  held = null;
  if (rectWaiting) return;
  rectWaiting = true;
  void call("spike_browser_rect", () => {
    rectWaiting = false;
    return rectArgs(el);
  }).catch(() => {});
}

// Rule mode (finding 21): the layout's inputs and the pane's top and bottom as last sent. The rule goes ahead of the report
// in the same queue, and only when one of them changed.
let ruleSent = "";
function sendRule(el: HTMLElement): void {
  const inputs = el.closest<HTMLElement>(".window")?.dataset.spikeRule;
  if (!inputs) return;
  const m = measure(el);
  const top = m.y;
  const bottom = m.winHeight - m.y - m.height;
  const key = `${inputs}|${top}|${bottom}`;
  if (key === ruleSent) return;
  ruleSent = key;
  const pieces = paneRule(JSON.parse(inputs) as RuleState);
  spikeLog("rule-sent", { inputs: JSON.parse(inputs), top, bottom, pieces: pieces.length });
  void call("spike_browser_rule", { rule: { pieces, top, bottom } }).catch(() => {});
}

/** The placeholder's box, and the size of the layout it sits in. That is the app's `.window`, which LiveApp sizes from a
 * ResizeObserver, so during a resize `innerWidth` runs a frame ahead of the layout (finding 17); predict mode learns from
 * pairs that match. */
function measure(el: HTMLElement) {
  const r = el.getBoundingClientRect();
  const layout = el.closest<HTMLElement>(".window")?.getBoundingClientRect();
  return { x: r.x, y: r.y, width: r.width, height: r.height, winWidth: layout?.width ?? window.innerWidth, winHeight: layout?.height ?? window.innerHeight };
}

/** Resolves after the next frame has been painted: a timeout queued from the frame's callback runs after that paint. */
function nextPaint(): Promise<void> {
  return new Promise((done) => requestAnimationFrame(() => setTimeout(done, 0)));
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
type PageState = { url: string; title: string; canBack: boolean; canForward: boolean };

export function BrowserSpike({ model, frozen }: Props) {
  const page = useRef<HTMLDivElement>(null);
  const [address, setAddress] = useState(model.request?.url ?? "");
  const [status, setStatus] = useState("");
  const [history, setHistory] = useState<PageState | null>(null);
  const { created, request } = model;
  // The page's picture while an app overlay is open; `src: null` is the plain fill a failed capture leaves.
  const [still, setStill] = useState<{ src: string | null } | null>(null);
  const freezeSeq = useRef(0);
  const freezing = useRef(false);

  // The overlay freeze. The page is a native window above the app's webview, so an overlay can't draw over it
  // (tauri#15682): while one is open, the page's picture takes its place, and only once that is on screen is the page hidden.
  useEffect(() => {
    if (!model.freeze || !created.current) return;
    const seq = ++freezeSeq.current;
    const stale = () => freezeSeq.current !== seq;
    const t0 = performance.now();
    const since = () => Math.round(performance.now() - t0);
    if (frozen) {
      freezing.current = true;
      spikeLog("freeze-start");
      void (async () => {
        let src: string | null = null;
        try {
          src = await call<string>("spike_browser_capture");
          spikeLog("captured", { ms: since(), chars: src.length });
          const img = new Image();
          img.src = src;
          await img.decode();
        } catch (e) {
          src = null;
          spikeLog("capture-failed", { ms: since(), error: String(e) });
        }
        if (stale()) return;
        setStill({ src });
        await nextPaint();
        if (stale()) return;
        spikeLog("painted", { ms: since() });
        await call("spike_browser_visible", { visible: false }).catch(() => {});
        spikeLog("hidden", { ms: since() });
      })();
    } else if (freezing.current) {
      freezing.current = false;
      void (async () => {
        await call("spike_browser_visible", { visible: true }).catch(() => {});
        await nextPaint();
        if (stale()) return;
        setStill(null);
        spikeLog("thawed", { ms: since() });
      })();
    }
  }, [frozen, model.freeze, created]);

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
      sendRule(el);
      reportRect(el);
    }
    const report = () => {
      if (!created.current) return;
      sendRule(el);
      reportRect(el);
    };
    // Only the placeholder's own ResizeObserver: the app lays itself out from a ResizeObserver too (LiveApp's
    // useElementSize), so at the window's resize event the DOM still has the old layout and a report from it is stale.
    const ro = new ResizeObserver(report);
    ro.observe(el);
    // A layout input that moves the pane without resizing it (the sidebar's width with the page full) fires no
    // ResizeObserver, so the inputs are watched too.
    const host = el.closest(".window");
    const mo = new MutationObserver(report);
    if (host) mo.observe(host, { attributes: true, attributeFilter: ["data-spike-rule"] });
    return () => {
      ro.disconnect();
      mo.disconnect();
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

  // The bar follows the page: its address, and whether Back and Forward can go anywhere.
  useEffect(() => {
    const offHistory = listen<PageState>("spike://history", ({ payload }) => {
      setHistory(payload);
      setAddress(payload.url);
    });
    const offPopup = listen<{ url: string }>("spike://popup", ({ payload }) => {
      setStatus(`A new-window link opened here: ${payload.url}`);
      spikeLog("popup-shown", { url: payload.url });
    });
    return () => {
      void offHistory.then((f) => f());
      void offPopup.then((f) => f());
    };
  }, []);

  const nav = (action: "back" | "forward" | "reload") => {
    setStatus("");
    call("spike_browser_nav", { action }).catch((e) => setStatus(String(e)));
  };

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
    const done = created.current ? call("spike_browser_nav", { action: "go", url: address }) : call("spike_browser_open", { url: address, rect: page.current ? measure(page.current) : null });
    done.catch((e) => setStatus(String(e)));
  };

  return (
    <section className="spike-browser" aria-label="Browser (spike)" style={{ "--spike-inset": `${model.inset}px` } as CSSProperties}>
      <div className="spike-browser__bar">
        <button type="button" disabled={!history?.canBack} onClick={() => nav("back")}>
          Back
        </button>
        <button type="button" disabled={!history?.canForward} onClick={() => nav("forward")}>
          Forward
        </button>
        <button type="button" disabled={!history} onClick={() => nav("reload")}>
          Reload
        </button>
        <input aria-label="Address" value={address} onChange={(e) => setAddress(e.target.value)} onKeyDown={(e) => e.key === "Enter" && go()} />
      </div>
      <p className="spike-browser__status">
        {status}
      </p>
      <div ref={page} className="spike-browser__page" tabIndex={0} aria-label="Web page" onFocus={onPageFocus}>
        {still && (still.src ? <img className="spike-browser__frozen" src={still.src} alt="" /> : <div className="spike-browser__frozen spike-browser__fill" />)}
      </div>
    </section>
  );
}
