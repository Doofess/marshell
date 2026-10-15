import "@xterm/xterm/css/xterm.css";
import { useEffect, useRef } from "react";
import { SCRIPTED_SESSION, SCRIPTED_SIZE } from "./scriptedSession";
import { appScheme, resolveScheme, terminalOptions, type TerminalThemeSetting } from "./terminalTheme";
import "./ScriptedTerminal.css";

/**
 * A real xterm.js fed canned Claude-style output. On the server it renders only the labelled host; the client
 * (Storybook, or the review page's inlined script) mounts xterm into it and follows the app theme live.
 * xterm is imported inside the effect: the library touches browser globals at load, so a top-level import would
 * break server rendering and the node test environment. The host is a group, not an img: xterm puts a focusable
 * input inside it.
 */
export function ScriptedTerminal({
  setting = "follow-app",
  label = "Terminal preview",
  script = SCRIPTED_SESSION,
  size = SCRIPTED_SIZE,
}: {
  setting?: TerminalThemeSetting;
  label?: string;
  /** Raw terminal output to play; the Claude-style session by default. */
  script?: string;
  /** The terminal's starting size; it fits the host afterwards. */
  size?: { cols: number; rows: number };
}) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let cancelled = false;
    let dispose = () => {};

    void (async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit")]);
      if (cancelled) return;
      const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
      const scheme = () => resolveScheme(setting, appScheme(el));
      const term = new Terminal({
        fontFamily,
        fontSize: 13,
        cols: size.cols,
        rows: size.rows,
        disableStdin: true,
        cursorBlink: false,
        // A plain shell shows where you type: a solid block at the prompt even while the terminal is not focused.
        cursorInactiveStyle: script === SCRIPTED_SESSION ? "outline" : "block",
        scrollback: 1000,
        ...terminalOptions(scheme()),
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(el);
      term.write(script);
      fit.fit();

      // The padded host takes the terminal's own background, so a terminal pinned light or dark has no mismatched edge.
      const apply = () => {
        const o = terminalOptions(scheme());
        term.options.theme = o.theme;
        term.options.minimumContrastRatio = o.minimumContrastRatio;
        el.style.backgroundColor = o.theme.background ?? "";
      };
      apply();
      const prefers = matchMedia("(prefers-color-scheme: dark)");
      prefers.addEventListener("change", apply);
      const watch = new MutationObserver(apply);
      watch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
      const resize = new ResizeObserver(() => fit.fit());
      resize.observe(el);

      dispose = () => {
        prefers.removeEventListener("change", apply);
        watch.disconnect();
        resize.disconnect();
        term.dispose();
      };
    })();

    return () => {
      cancelled = true;
      dispose();
    };
  }, [setting, script, size.cols, size.rows]);

  // Another canned session (the shell panel's) travels in the markup, so the review page's script mounts that one.
  const custom = script !== SCRIPTED_SESSION;
  return (
    <div
      ref={host}
      className="scripted-terminal"
      role="group"
      aria-label={label}
      data-setting={setting}
      data-script={custom ? JSON.stringify(script) : undefined}
      data-cols={custom ? size.cols : undefined}
      data-rows={custom ? size.rows : undefined}
    />
  );
}
