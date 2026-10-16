import type { IMarker, Terminal } from "@xterm/xterm";

/** One OSC 133 shell-integration mark: A prompt start, B command start, C command executed, D command finished. */
export type Osc133 = { kind: "A" | "B" | "C" | "D"; exit: number | null };

/** Reads the text after "133;" ("A", "D;1", "D;0;aid=2"). Null for a mark we don't know. Only D carries an exit code. */
export function parseOsc133(data: string): Osc133 | null {
  const [kind, code] = data.split(";");
  if (kind !== "A" && kind !== "B" && kind !== "C" && kind !== "D") return null;
  const exit = kind === "D" && code !== undefined && /^-?\d+$/.test(code) ? Number(code) : null;
  return { kind, exit };
}

/** A shell prompt the output marked (OSC 133;A), and the exit code of the command typed at it once it finished. */
export type Prompt = { marker: IMarker; exit: number | null };

/** The part of xterm's Terminal the prompt tracker uses. */
export type PromptTerm = Pick<Terminal, "parser" | "registerMarker" | "registerDecoration">;

/** Remembers each prompt's line from the OSC 133 marks, and marks a finished command's exit code beside its prompt. */
export function trackPrompts(term: PromptTerm): { prompts: Prompt[]; reset: () => void; dispose: () => void } {
  const prompts: Prompt[] = [];
  let current: Prompt | null = null;
  let ran = false;
  const osc = term.parser.registerOscHandler(133, (data) => {
    const mark = parseOsc133(data);
    if (mark?.kind === "A") {
      const marker = term.registerMarker(0);
      // A user prompt with its own shell integration (oh-my-posh, WezTerm) sends a second A on the same line: one prompt.
      if (marker && current && current.marker.line === marker.line) marker.dispose();
      else if (marker) {
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
      const code = mark.exit;
      current.exit = code;
      const decoration = term.registerDecoration({ marker: current.marker, x: 0, width: 1 });
      decoration?.onRender((el) => {
        // Add to xterm's own classes: its xterm-decoration class is what places the element at the prompt.
        el.classList.add("prompt-mark");
        el.classList.toggle("prompt-mark--failed", code !== 0);
        el.setAttribute("role", "img");
        el.setAttribute("aria-label", `Exit code ${code}`);
        el.title = `Exit code ${code}`;
      });
      ran = false;
    }
    // Other handlers still see the sequence.
    return false;
  });
  return {
    prompts,
    reset: () => {
      prompts.length = 0;
      current = null;
    },
    dispose: () => osc.dispose(),
  };
}

/** The buffer line to scroll to, or null when there is no prompt in that direction. */
export function promptTarget(promptLines: number[], viewportTop: number, by: -1 | 1): number | null {
  if (by === -1) {
    const above = promptLines.filter((line) => line < viewportTop);
    return above.length === 0 ? null : Math.max(...above);
  }
  const below = promptLines.filter((line) => line > viewportTop);
  return below.length === 0 ? null : Math.min(...below);
}

/** Where a jump counts from. A prompt on the last screen cannot scroll to the top, so the view stops short of it
    (at baseY): while the view is still there, count from the prompt itself, or "next" would land on it again. */
export function jumpFrom(landed: number | null, baseY: number, viewportY: number): number {
  return landed !== null && Math.min(landed, baseY) === viewportY ? landed : viewportY;
}

/** What a prompt jump announces. A zero exit is not news, so only a failing one is said. */
export function jumpWords(by: -1 | 1, result: { index: number; total: number; exit: number | null } | null): string {
  if (!result) return by < 0 ? "No earlier prompt" : "No later prompt";
  return `Prompt ${result.index} of ${result.total}${result.exit !== null && result.exit !== 0 ? `, exit ${result.exit}` : ""}`;
}
