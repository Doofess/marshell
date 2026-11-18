import type { LineTokens, Tokenizer } from "../preview/highlight";

/** Colouring works in steps of at most this many lines, then yields. */
export const STEP_LINES = 100;
/** Above this many bytes of text there are no colours. */
export const COLOUR_CAP = 1024 * 1024;
/** A line over this many characters is not coloured. */
export const LONG_LINE = 2000;

/**
 * The colours of a text that changes, line by line.
 *
 * Colours come in order from the top, because a line's colours depend on the lines above it. The lines
 * that have colours are always the first `done` lines. The grammar state is kept at every STEP_LINES
 * boundary, and each step starts at one, so an edit restarts from the boundary at or before it.
 * `line` never answers with colours computed from an older text.
 */
export class TokenStore {
  /** The lines on screen. Every line above the first stale one has to be coloured first, so it only tells the caller what it asked for. */
  wanted: readonly [number, number] = [0, 0];

  private lines: string[] = [];
  private tokens: LineTokens[] = [];
  // checkpoints[k] is the grammar state before line k * STEP_LINES. It exists while that line is at or below `done`.
  private checkpoints: unknown[] = [undefined];
  private ver = 0;

  constructor(private readonly tokenize: Tokenizer) {}

  /** The number of lines that have colours: lines 0 to done - 1. */
  private get done(): number {
    return this.tokens.length;
  }

  get version(): number {
    return this.ver;
  }

  reset(lines: string[]): void {
    this.lines = lines.slice();
    this.tokens = [];
    this.checkpoints = [undefined];
    this.ver++;
  }

  edit(fromLine: number, removed: number, inserted: string[]): void {
    const from = Math.max(0, Math.min(fromLine, this.lines.length));
    this.lines.splice(from, Math.max(0, removed), ...inserted);
    if (this.done > from) this.tokens.length = from;
    this.checkpoints.length = Math.floor(this.done / STEP_LINES) + 1;
    this.ver++;
  }

  want(fromLine: number, toLine: number): void {
    this.wanted = [fromLine, toLine];
  }

  /** Colours up to STEP_LINES lines. False when nothing is stale. */
  step(): boolean {
    if (this.done >= this.lines.length) return false;
    const start = Math.floor(this.done / STEP_LINES) * STEP_LINES;
    const end = Math.min(start + STEP_LINES, this.lines.length);
    const result = this.tokenize(this.lines.slice(start, end), this.checkpoints[start / STEP_LINES]);
    const had = this.done;
    for (let i = had - start; i < result.tokens.length; i++) this.tokens[start + i] = result.tokens[i]!;
    if (end - start === STEP_LINES) this.checkpoints[end / STEP_LINES] = result.state;
    if (end > had) this.ver++;
    return true;
  }

  line(n: number): LineTokens | null {
    return n >= 0 && n < this.done ? (this.tokens[n] ?? null) : null;
  }
}
