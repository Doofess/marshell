import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { findCommand } from "../../lib/bindings";
import { osOfPlatform } from "../../lib/keymap";
import { onCoreEvent } from "../../lib/api/coreEvents";
import { PtySocket } from "../../lib/api/ptySocket";
import { LatencyMeter } from "../perf/latency";
import { PerfOverlay } from "../perf/PerfOverlay";
import { ENDED_LINE, FAILED_LINE, SESSION_ENDED } from "./coreRestart";
import { loadRenderer, type RendererKind } from "./renderer";
import { appScheme, resolveScheme, terminalOptions } from "./terminalTheme";

function isPerfToggle(e: KeyboardEvent): boolean {
  return e.type === "keydown" && e.shiftKey && e.altKey && (e.ctrlKey || e.metaKey) && e.code === "KeyP";
}

/**
 * One session's terminal. `placeholder` is a line shown over it until the first output arrives ("Starting Claude Code in
 * my-app…"). `onSize` hears the terminal's size once it is measured and whenever it changes.
 */
export function TerminalView({ endpoint, tabId, placeholder, onSize }: { endpoint: Endpoint; tabId: string; placeholder?: string; onSize?: (cols: number, rows: number) => void }) {
  const host = useRef<HTMLDivElement>(null);
  const meter = useMemo(() => new LatencyMeter(), []);
  const [renderer, setRenderer] = useState<RendererKind>("dom");
  const [perfOpen, setPerfOpen] = useState(false);
  const [gotOutput, setGotOutput] = useState(false);
  // The effect below must not restart when the parent passes a new callback.
  const sizeRef = useRef(onSize);
  sizeRef.current = onSize;

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
    const apply = () => terminalOptions(resolveScheme("follow-app", appScheme()));
    const term = new Terminal({ fontFamily, fontSize: 13, cursorBlink: true, scrollback: 10_000, ...apply() });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    loadRenderer(term, endpoint.platform, setRenderer);
    term.attachCustomKeyEventHandler((e) => {
      if (isPerfToggle(e)) {
        setPerfOpen((open) => !open);
        return false;
      }
      // The launcher shortcut belongs to the app: the terminal must not type it (Ctrl+T is a control code).
      if (e.type === "keydown" && findCommand(e, osOfPlatform(endpoint.platform))?.id === "launcher") return false;
      return true;
    });

    const socket = new PtySocket(endpoint, tabId, {
      onOutput: (seq, bytes) => {
        if (bytes.length > 0) setGotOutput(true);
        term.write(bytes, () => socket.processed(seq + bytes.length));
      },
      onReset: (seq, screen) => {
        if (screen.length > 0) setGotOutput(true);
        term.reset();
        term.write(screen, () => socket.processed(seq));
      },
      onExit: (code) => term.write(`\r\n\x1b[2mProcess exited with code ${code}. ${SESSION_ENDED}\x1b[0m\r\n`),
    });

    // After a core restart ("ended") or failure ("failed") the session is gone, so no input goes anywhere.
    let core: "live" | "ended" | "failed" = "live";
    const stopCoreEvents = onCoreEvent((e) => {
      if (core !== "live") return;
      core = e.kind === "restarted" ? "ended" : "failed";
      socket.close();
      term.write(e.kind === "restarted" ? ENDED_LINE : FAILED_LINE);
    }).catch(() => () => {}); // Outside Tauri (tests, a plain browser) there are no core events.

    const subs = [
      term.onData((d) => {
        if (core !== "live") return;
        meter.markInput();
        socket.sendInput(d);
      }),
      term.onBinary((d) => core === "live" && socket.sendBytes(d)),
      term.onResize(({ cols, rows }) => {
        socket.resize(cols, rows);
        sizeRef.current?.(cols, rows);
      }),
      term.onWriteParsed(() => meter.markOutput()),
    ];

    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => fit.fit());
    });
    observer.observe(el);
    fit.fit();
    socket.resize(term.cols, term.rows);
    sizeRef.current?.(term.cols, term.rows);
    term.focus();

    const scheme = matchMedia("(prefers-color-scheme: dark)");
    const onScheme = () => {
      const o = apply();
      term.options.theme = o.theme;
      term.options.minimumContrastRatio = o.minimumContrastRatio;
    };
    scheme.addEventListener("change", onScheme);
    const themeWatch = new MutationObserver(onScheme);
    themeWatch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    return () => {
      void stopCoreEvents.then((stop) => stop());
      themeWatch.disconnect();
      scheme.removeEventListener("change", onScheme);
      observer.disconnect();
      cancelAnimationFrame(frame);
      subs.forEach((s) => s.dispose());
      socket.close();
      term.dispose();
    };
  }, [endpoint, tabId, meter]);

  return (
    <>
      <div ref={host} className="terminal-host" />
      {placeholder && !gotOutput && (
        <p className="terminal-placeholder">{placeholder}</p>
      )}
      {perfOpen && <PerfOverlay meter={meter} renderer={renderer} />}
    </>
  );
}
