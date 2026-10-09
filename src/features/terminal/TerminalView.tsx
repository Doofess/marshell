import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { PtySocket } from "../../lib/api/ptySocket";
import { LatencyMeter } from "../perf/latency";
import { PerfOverlay } from "../perf/PerfOverlay";
import { loadRenderer, type RendererKind } from "./renderer";
import { currentTheme } from "./theme";

function isPerfToggle(e: KeyboardEvent): boolean {
  return e.type === "keydown" && e.shiftKey && e.altKey && (e.ctrlKey || e.metaKey) && e.code === "KeyP";
}

export function TerminalView({ endpoint, tabId }: { endpoint: Endpoint; tabId: string }) {
  const host = useRef<HTMLDivElement>(null);
  const meter = useMemo(() => new LatencyMeter(), []);
  const [renderer, setRenderer] = useState<RendererKind>("dom");
  const [perfOpen, setPerfOpen] = useState(false);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
    const term = new Terminal({ fontFamily, fontSize: 13, cursorBlink: true, scrollback: 10_000, theme: currentTheme() });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    loadRenderer(term, endpoint.platform, setRenderer);
    term.attachCustomKeyEventHandler((e) => {
      if (isPerfToggle(e)) {
        setPerfOpen((open) => !open);
        return false;
      }
      return true;
    });

    const socket = new PtySocket(endpoint, tabId, {
      onOutput: (seq, bytes) => term.write(bytes, () => socket.processed(seq + bytes.length)),
      onReset: (seq, screen) => {
        term.reset();
        term.write(screen, () => socket.processed(seq));
      },
      onExit: (code) => term.write(`\r\n\x1b[2mProcess exited with code ${code}.\x1b[0m\r\n`),
    });

    const subs = [
      term.onData((d) => {
        meter.markInput();
        socket.sendInput(d);
      }),
      term.onBinary((d) => socket.sendBytes(d)),
      term.onResize(({ cols, rows }) => socket.resize(cols, rows)),
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
    term.focus();

    const scheme = matchMedia("(prefers-color-scheme: dark)");
    const onScheme = () => {
      term.options.theme = currentTheme();
    };
    scheme.addEventListener("change", onScheme);

    return () => {
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
      {perfOpen && <PerfOverlay meter={meter} renderer={renderer} />}
    </>
  );
}
