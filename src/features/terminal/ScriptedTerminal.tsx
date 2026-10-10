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
export function ScriptedTerminal({ setting = "follow-app", label = "Terminal preview" }: { setting?: TerminalThemeSetting; label?: string }) {
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
        cols: SCRIPTED_SIZE.cols,
        rows: SCRIPTED_SIZE.rows,
        disableStdin: true,
        cursorBlink: false,
        scrollback: 1000,
        ...terminalOptions(scheme()),
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(el);
      term.write(SCRIPTED_SESSION);
      fit.fit();

      const apply = () => {
        const o = terminalOptions(scheme());
        term.options.theme = o.theme;
        term.options.minimumContrastRatio = o.minimumContrastRatio;
      };
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
  }, [setting]);

  return <div ref={host} className="scripted-terminal" role="group" aria-label={label} data-setting={setting} />;
}
