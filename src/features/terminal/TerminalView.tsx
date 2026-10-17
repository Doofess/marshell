import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal, type ITerminalOptions } from "@xterm/xterm";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { sessionShortcut } from "../../lib/appShortcuts";
import { osOfPlatform } from "../../lib/keymap";
import { onCoreEvent } from "../../lib/api/coreEvents";
import { claimsKey } from "../sessions/useNavigation";
import { PtySocket } from "../../lib/api/ptySocket";
import { LatencyMeter } from "../perf/latency";
import { useOpenPreview } from "../preview/PreviewContext";
import { PerfOverlay } from "../perf/PerfOverlay";
import { ENDED_LINE, FAILED_LINE, SESSION_ENDED } from "./coreRestart";
import { jumpFrom, promptTarget, trackPrompts, type Prompt } from "./promptMarks";
import "./PromptMarks.css";
import { pathLinkProvider } from "./pathLinks";
import { loadRenderer, type RendererKind } from "./renderer";
import { trackColorSchemeMode } from "./colorSchemeMode";
import { BUNDLED_FONT, fontStack } from "./terminalFont";
import { appScheme, resolveTerminalTheme, type TerminalThemeInput } from "./terminalTheme";

/** The terminal theme settings, without the app's scheme, which the terminal reads itself. */
export type TerminalThemeProp = Omit<TerminalThemeInput, "app">;
const FOLLOW_APP: TerminalThemeProp = { setting: "follow-app", imported: [], protection: true };

function isPerfToggle(e: KeyboardEvent): boolean {
  return e.type === "keydown" && e.shiftKey && e.altKey && (e.ctrlKey || e.metaKey) && e.code === "KeyP";
}

/** Whether a key press is turned into the "Paused. Continue to type." bar instead of being sent: a live session that is frozen. */
export const blockedInput = (core: "live" | "ended" | "failed", paused: boolean): boolean => core === "live" && paused;

/** Focus reports (`CSI I`/`CSI O`) and mouse reports (SGR, or X10's `CSI M` plus three bytes): the terminal sends these itself, so they are not typing. */
export const isTerminalReport = (data: string): boolean => /^\x1b\[(?:[IO]|<\d+;\d+;\d+[Mm]|M[\s\S]{3})$/.test(data);

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
  /** True while the session is paused: typing sends nothing and calls `onBlockedInput`. */
  paused?: boolean;
  /** Hears a key press that was not sent because the session is paused. */
  onBlockedInput?: () => void;
  /** Each new token scrolls once to the previous (-1) or next (1) shell prompt. */
  promptJump?: { token: number; by: -1 | 1 };
  /** Hears where a prompt jump landed (1-based index, count, the exit code of that prompt's command), or null when there was no prompt that way. */
  onPromptJump?: (result: { index: number; total: number; exit: number | null } | null) => void;
  /** The Appearance terminal theme and this profile's override. A change applies at once. Default: follow the app. */
  theme?: TerminalThemeProp;
  /** The resolved terminal font (terminalFont.ts). A change applies at once. Default: the bundled font at 13 px. */
  font?: { family: string; size: number };
};

const BUNDLED = { family: BUNDLED_FONT, size: 13 };

/** The options every session's terminal starts with. The exit-code marks (promptMarks.ts) draw with registerDecoration, a proposed API. */
export function terminalOptions(o: Pick<ITerminalOptions, "fontFamily" | "fontSize" | "theme" | "minimumContrastRatio">): ITerminalOptions {
  return { ...o, cursorBlink: true, scrollback: 10_000, allowProposedApi: true };
}

/** Waits for a font so xterm measures its cells with the right glyphs. Gives up after a second; a missing font is not an error. */
function loadFont(family: string, size: number): Promise<void> {
  const load = document.fonts.load(`${size}px ${fontStack(family)}`).then(() => {});
  const timeout = new Promise<void>((done) => setTimeout(done, 1000));
  return Promise.race([load, timeout]).catch(() => {});
}

/** One session's terminal. */
export function TerminalView({ endpoint, tabId, placeholder, onOutput, onSize, autoFocus = true, focusToken = 0, visible = true, paused = false, onBlockedInput, promptJump, onPromptJump, theme = FOLLOW_APP, font = BUNDLED }: Props) {
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
  const pausedRef = useRef(paused);
  pausedRef.current = paused;
  const blockedRef = useRef(onBlockedInput);
  blockedRef.current = onBlockedInput;
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const promptsRef = useRef<Prompt[]>([]);
  const jumpRef = useRef(onPromptJump);
  jumpRef.current = onPromptJump;
  const lastJump = useRef<number | null>(null);
  const landedRef = useRef<number | null>(null);
  // Ctrl+click on a path in the output opens it in the preview; the terminal keeps the latest opener without restarting.
  const openPreview = useOpenPreview();
  const previewRef = useRef(openPreview);
  previewRef.current = openPreview;
  const themeRef = useRef(theme);
  themeRef.current = theme;
  // Re-applies the theme to the live terminal; set while a terminal exists.
  const restyleRef = useRef<(() => void) | null>(null);
  const fontRef = useRef(font);
  fontRef.current = font;
  // The first terminal waits for its font once; later font changes apply to the running terminal.
  const [fontReady, setFontReady] = useState(false);
  useEffect(() => {
    let live = true;
    void loadFont(fontRef.current.family, fontRef.current.size).then(() => live && setFontReady(true));
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    const el = host.current;
    if (!el || !fontReady) return;
    const fontFamily = fontStack(fontRef.current.family);
    const fontSize = fontRef.current.size;
    const resolve = () => resolveTerminalTheme({ ...themeRef.current, app: appScheme() });
    const first = resolve();
    // The gutter around the cells (app.css) matches the theme's background.
    const paintGutter = (background: string | undefined) => el.style.setProperty("--terminal-bg", background ?? null);
    paintGutter(first.theme.background);
    let current = first.scheme;
    const term = new Terminal(terminalOptions({ fontFamily, fontSize, theme: first.theme, minimumContrastRatio: first.minimumContrastRatio }));
    const fit = new FitAddon();
    term.loadAddon(fit);
    fitRef.current = fit;
    term.open(el);
    termRef.current = term;
    loadRenderer(term, endpoint.platform, setRenderer);
    // The provider is disposed with the terminal.
    term.registerLinkProvider(pathLinkProvider(term, tabId, endpoint.platform, (entry) => previewRef.current(entry)));
    term.attachCustomKeyEventHandler((e) => {
      if (isPerfToggle(e)) {
        setPerfOpen((open) => !open);
        return false;
      }
      // The app's session shortcuts (launcher, close, reopen, rename, go to, move) belong to the app: the terminal must not type
      // them (Ctrl+T and Ctrl+W are control codes). The app hears the same key on the window.
      if (e.type === "keydown" && sessionShortcut(e, osOfPlatform(endpoint.platform)) !== null) return false;
      // So do the navigation chords once navigation has started (Ctrl+Shift+arrow selects a word until then).
      if (e.type === "keydown" && claimsKey(e, osOfPlatform(endpoint.platform), "terminal")) return false;
      return true;
    });

    // Shell integration marks (OSC 133): remember each prompt's line, and mark a finished command's exit code beside it.
    const tracker = trackPrompts(term);
    promptsRef.current = tracker.prompts;
    // Mode 2031: tell a program that asked when the terminal switches between dark and light.
    const schemeMode = trackColorSchemeMode(term, (d) => core === "live" && socket.sendInput(d), () => current);

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
        tracker.reset();
        landedRef.current = null;
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
        if (blockedInput(core, pausedRef.current)) return isTerminalReport(d) ? undefined : blockedRef.current?.();
        meter.markInput();
        socket.sendInput(d);
      }),
      term.onBinary((d) => core === "live" && !pausedRef.current && socket.sendBytes(d)),
      term.onResize(({ cols, rows }) => {
        socket.resize(cols, rows);
        sizeRef.current?.(cols, rows);
      }),
      term.onWriteParsed(() => meter.markOutput()),
      tracker,
      schemeMode,
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
      const o = resolve();
      term.options.theme = o.theme;
      paintGutter(o.theme.background);
      term.options.minimumContrastRatio = o.minimumContrastRatio;
      current = o.scheme;
      schemeMode.notify(o.scheme);
    };
    restyleRef.current = onScheme;
    scheme.addEventListener("change", onScheme);
    const themeWatch = new MutationObserver(onScheme);
    themeWatch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    return () => {
      void stopCoreEvents.then((stop) => stop());
      restyleRef.current = null;
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
  }, [endpoint, tabId, meter, fontReady]);

  // A changed font applies to the running terminal once it has loaded, then the grid is re-fitted to the new cell size.
  useEffect(() => {
    let live = true;
    void loadFont(font.family, font.size).then(() => {
      const term = termRef.current;
      if (!live || !term) return;
      term.options.fontFamily = fontStack(font.family);
      term.options.fontSize = font.size;
      fitRef.current?.fit();
    });
    return () => {
      live = false;
    };
  }, [font.family, font.size]);

  // A changed setting or override restyles the running terminal; no restart.
  useEffect(() => {
    restyleRef.current?.();
  }, [theme]);

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
    const from = jumpFrom(landedRef.current, buffer.baseY, buffer.viewportY);
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
