import { SESSION_ENDED } from "./coreRestart";

/** The core's exit frame: the terminal says the process ended, then, once that line is written, its owner hears of it (`onExited`). */
export function processExited(write: (text: string, done: () => void) => void, code: number, exited: ((code: number) => void) | undefined): void {
  write(`\r\n\x1b[2mProcess exited with code ${code}. ${SESSION_ENDED}\x1b[0m\r\n`, () => exited?.(code));
}
