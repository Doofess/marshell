// Spike 6q-0 (never merged): the browser pane in the preview slot. Rust draws the page as a child webview over the
// placeholder; this side only measures the placeholder and drives the child through the spike commands.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type CSSProperties } from "react";
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

  useEffect(() => {
    const off = listen<{ url: string }>("spike://refused", (e) => {
      setStatus(`Refused ${e.payload.url}`);
      spikeLog("refused-shown", { url: e.payload.url });
    });
    return () => void off.then((f) => f());
  }, []);

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
      <div ref={page} className="spike-browser__page" tabIndex={0} aria-label="Web page" />
    </section>
  );
}
