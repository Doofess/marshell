import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal, type IMarker } from "@xterm/xterm";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { sessionShortcut } from "../../lib/appShortcuts";
import { osOfPlatform } from "../../lib/keymap";
import { onCoreEvent } from "../../lib/api/coreEvents";
import { PtySocket } from "../../lib/api/ptySocket";
import { LatencyMeter } from "../perf/latency";
import { PerfOverlay } from "../perf/PerfOverlay";
import { ENDED_LINE, FAILED_LINE, SESSION_ENDED } from "./coreRestart";
import { parseOsc133, promptTarget } from "./promptMarks";
import "./PromptMarks.css";
import { loadRenderer, type RendererKind } from "./renderer";
import { appScheme, resolveScheme, terminalOptions } from "./terminalTheme";

function isPerfToggle(e: KeyboardEvent): boolean {
  return e.type === "keydown" && e.shiftKey && e.altKey && (e.ctrlKey || e.metaKey) && e.code === "KeyP";
}

type Props = {
  endpoint: Endpoint;
  tabId: string;
  /** A line shown over the terminal until the first output arrives ("Starting Claude Code in my-app…"). */
  placeholder?: string;
  /** Hears the first output. The owner of `placeholder` drops it then, so it stays gone when this terminal is mounted again. */
  onOutput?: () => void;
  /** Hears the terminal's size once it is measured and whenever it changes. */
  onSize?: (cols: number, rows: number) => void;
  /** False while something else (the launcher) has the keys: the terminal takes focus when this turns true. Default true. */
  autoFocus?: boolean;
  /** Bumping this takes focus back to the terminal (a closed question or a finished rename hands the keys back). */
  focusToken?: number;
  /** False while the terminal is kept mounted but hidden behind another session's. It fits and takes focus when this turns true. Default true. */
  visible?: boolean;
  /** Each new token scrolls once to the previous (-1) or next (1) shell prompt. */
  promptJump?: { token: number; by: -1 | 1 };
  /** Hears where a prompt jump landed (1-based index, count, the exit code of that prompt's command), or null when there was no prompt that way. */
  onPromptJump?: (result: { index: number; total: number; exit: number | null } | null) => void;
};

/** A shell prompt the output marked (OSC 133;A), and the exit code of the command typed at it once it finished. */
type Prompt = { marker: IMarker; exit: number | null };

/** One session's terminal. */
export function TerminalView({ endpoint, tabId, placeholder, onOutput, onSize, autoFocus = true, focusToken = 0, visible = true, promptJump, onPromptJump }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const meter = useMemo(() => new LatencyMeter(), []);
  const [renderer, setRenderer] = useState<RendererKind>("dom");
  const [perfOpen, setPerfOpen] = useState(false);
  const [gotOutput, setGotOutput] = useState(false);
  // The effect below must not restart when the parent passes a new callback.
  const sizeRef = useRef(onSize);
  sizeRef.current = onSize;
  const outputRef = useRef(onOutput);
  outputRef.current = onOutput;
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const promptsRef = useRef<Prompt[]>([]);
  const jumpRef = useRef(onPromptJump);
  jumpRef.current = onPromptJump;
  const lastJump = useRef<number | null>(null);
  const landedRef = useRef<number | null>(null);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
    const apply = () => terminalOptions(resolveScheme("follow-app", appScheme()));
    const term = new Terminal({ fontFamily, fontSize: 13, cursorBlink: true, scrollback: 10_000, ...apply() });
    const fit = new FitAddon();
    term.loadAddon(fit);
    fitRef.current = fit;
    term.open(el);
    termRef.current = term;
    loadRenderer(term, endpoint.platform, setRenderer);
    term.attachCustomKeyEventHandler((e) => {
      if (isPerfToggle(e)) {
        setPerfOpen((open) => !open);
        return false;
      }
      // The app's session shortcuts (launcher, close, reopen, rename, go to, move) belong to the app: the terminal must not type
      // them (Ctrl+T and Ctrl+W are control codes). The app hears the same key on the window.
      if (e.type === "keydown" && sessionShortcut(e, osOfPlatform(endpoint.platform)) !== null) return false;
      return true;
    });

    // Shell integration marks (OSC 133): remember each prompt's line, and mark a finished command's exit code beside it.
    const prompts: Prompt[] = [];
    promptsRef.current = prompts;
    let current: Prompt | null = null;
    let ran = false;
    const osc = term.parser.registerOscHandler(133, (data) => {
      const mark = parseOsc133(data);
      if (mark?.kind === "A") {
        const marker = term.registerMarker(0);
        if (marker) {
          const prompt: Prompt = { marker, exit: null };
          prompts.push(prompt);
          // A marker trimmed off the top of the scrollback is gone; so is its prompt.
          marker.onDispose(() => {
            const i = prompts.indexOf(prompt);
            if (i >= 0) prompts.splice(i, 1);
          });
          current = prompt;
        }
        ran = false;
      } else if (mark?.kind === "C") {
        ran = true;
      } else if (mark?.kind === "D" && mark.exit !== null && ran && current) {
        const prompt = current;
        const code = mark.exit;
        prompt.exit = code;
        const decoration = term.registerDecoration({ marker: prompt.marker, x: 0, width: 1 });
        decoration?.onRender((el) => {
          el.className = code === 0 ? "prompt-mark" : "prompt-mark prompt-mark--failed";
          el.setAttribute("role", "img");
          el.setAttribute("aria-label", `Exit code ${code}`);
          el.title = `Exit code ${code}`;
        });
        ran = false;
      }
      // Other handlers still see the sequence.
      return false;
    });

    const socket = new PtySocket(endpoint, tabId, {
      onOutput: (seq, bytes) => {
        if (bytes.length > 0) {
          setGotOutput(true);
          outputRef.current?.();
        }
        term.write(bytes, () => socket.processed(seq + bytes.length));
      },
      onReset: (seq, screen) => {
        if (screen.length > 0) {
          setGotOutput(true);
          outputRef.current?.();
        }
        term.reset();
        prompts.length = 0;
        current = null;
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
      osc,
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
      termRef.current = null;
      fitRef.current = null;
    };
  }, [endpoint, tabId, meter]);

  // Declared after the effect above so a new terminal exists by the time this runs.
  useEffect(() => {
    if (!visible) return;
    // Shown again after being hidden: the window may have changed size meanwhile, and a hidden host measures as zero.
    fitRef.current?.fit();
    if (autoFocus) termRef.current?.focus();
  }, [autoFocus, visible, endpoint, tabId, focusToken]);

  useEffect(() => {
    if (!promptJump || promptJump.token === lastJump.current) return;
    lastJump.current = promptJump.token;
    const term = termRef.current;
    if (!term) return;
    const live = promptsRef.current.filter((p) => p.marker.line >= 0);
    const buffer = term.buffer.active;
    // A prompt on the last screen cannot scroll to the top, so the view stops short of it; count from the prompt itself then.
    const landed = landedRef.current;
    const from = landed !== null && Math.min(landed, buffer.baseY) === buffer.viewportY ? landed : buffer.viewportY;
    const target = promptTarget(
      live.map((p) => p.marker.line),
      from,
      promptJump.by,
    );
    if (target === null) return jumpRef.current?.(null);
    landedRef.current = target;
    term.scrollToLine(target);
    const index = live.findIndex((p) => p.marker.line === target);
    jumpRef.current?.({ index: index + 1, total: live.length, exit: live[index]?.exit ?? null });
  }, [promptJump]);

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
