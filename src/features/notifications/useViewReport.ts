import { useEffect, useRef } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { ui, type ViewReport } from "../../lib/api/ui";

/** The core refuses a report naming more tabs than this. */
const MAX_TABS = 16;
const MIN_GAP_MS = 200;

type Send = (ep: Endpoint, report: ViewReport) => Promise<void>;

/**
 * Sends the view report to one core: at most once per 200 ms (the first at once, a burst's last after the gap) and never the
 * value it last sent. A failed send is forgotten, so the next change sends again.
 */
class ViewReporter {
  private lastKey: string | null = null;
  private lastAt = -Infinity;
  private pending: ViewReport | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private stopped = false;

  constructor(
    private ep: Endpoint,
    private send: Send,
  ) {}

  update(report: ViewReport): void {
    this.pending = { window_focused: report.window_focused, visible_tabs: report.visible_tabs.slice(0, MAX_TABS) };
    if (this.timer !== undefined) return;
    const wait = this.lastAt + MIN_GAP_MS - Date.now();
    if (wait <= 0) this.flush();
    else this.timer = setTimeout(() => this.flush(), wait);
  }

  stop(): void {
    this.stopped = true;
    clearTimeout(this.timer);
    this.timer = undefined;
  }

  private flush(): void {
    this.timer = undefined;
    const report = this.pending;
    this.pending = null;
    if (this.stopped || report === null) return;
    const key = JSON.stringify(report);
    if (key === this.lastKey) return;
    this.lastKey = key;
    this.lastAt = Date.now();
    this.send(this.ep, report).catch(() => {
      if (this.lastKey === key) this.lastKey = null;
    });
  }
}

/**
 * Tells the core whether the window has focus and which sessions are on screen. A restarted core hands over a new endpoint and
 * knows nothing yet, so it gets the report again.
 */
export function useViewReport(endpoint: Endpoint | null | undefined, visibleTabs: string[], send: Send = ui.putView): void {
  const tabs = useRef(visibleTabs);
  tabs.current = visibleTabs;
  const reporter = useRef<ViewReporter | null>(null);
  const key = visibleTabs.join("\n");

  useEffect(() => {
    if (!endpoint) return;
    const r = new ViewReporter(endpoint, send);
    reporter.current = r;
    const report = () => r.update({ window_focused: document.hasFocus(), visible_tabs: tabs.current });
    report();
    window.addEventListener("focus", report);
    window.addEventListener("blur", report);
    return () => {
      window.removeEventListener("focus", report);
      window.removeEventListener("blur", report);
      r.stop();
      reporter.current = null;
    };
  }, [endpoint, send]);

  useEffect(() => {
    reporter.current?.update({ window_focused: document.hasFocus(), visible_tabs: tabs.current });
  }, [key]);
}
