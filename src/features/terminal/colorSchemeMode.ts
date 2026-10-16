import type { IDisposable, Terminal } from "@xterm/xterm";
import type { Scheme } from "./palettes";

/** The part of xterm's Terminal the colour-scheme tracker uses. */
export type SchemeTerm = { parser: Pick<Terminal["parser"], "registerCsiHandler" | "registerEscHandler"> };

/** The colour-scheme report: CSI ? 997 ; 1 n for dark, 2 for light. */
const report = (scheme: Scheme) => `\x1b[?997;${scheme === "dark" ? 1 : 2}n`;

const has = (params: (number | number[])[], n: number) => params.some((p) => p === n);

/**
 * Mode 2031 (the colour-scheme update notice, from Contour): a program that turns it on is told each time the terminal
 * switches between dark and light. CSI ? 996 n asks for the current scheme at any time.
 */
export function trackColorSchemeMode(term: SchemeTerm, send: (data: string) => void, current: () => Scheme): { notify(scheme: Scheme): void; dispose(): void } {
  let on = false;
  let last: Scheme | null = null;
  const subs: IDisposable[] = [
    term.parser.registerCsiHandler({ prefix: "?", final: "h" }, (params) => {
      if (has(params, 2031)) {
        on = true;
        last = current();
      }
      // xterm still handles the other modes in the same sequence.
      return false;
    }),
    term.parser.registerCsiHandler({ prefix: "?", final: "l" }, (params) => {
      if (has(params, 2031)) on = false;
      return false;
    }),
    term.parser.registerCsiHandler({ prefix: "?", final: "n" }, (params) => {
      if (!has(params, 996)) return false;
      last = current();
      send(report(last));
      return true;
    }),
    // A full reset (RIS) turns every mode off.
    term.parser.registerEscHandler({ final: "c" }, () => {
      on = false;
      return false;
    }),
  ];
  return {
    notify(scheme) {
      if (!on || scheme === last) return;
      last = scheme;
      send(report(scheme));
    },
    dispose: () => subs.forEach((s) => s.dispose()),
  };
}
