import { describe, expect, it, vi } from "vitest";
import { focusShellTerminal, shellTerminalFocused, SHELL_TERMINAL_INPUT } from "./shellPanelFocus";

type Fake = { classList: { contains: (c: string) => boolean }; closest: (s: string) => unknown; focus: () => void };
const input = (inPanel: boolean, focus = vi.fn()): Fake => ({
  classList: { contains: (c) => c === "xterm-helper-textarea" },
  closest: (s) => (inPanel && s === ".shell-panel" ? {} : null),
  focus,
});

describe("shellTerminalFocused", () => {
  it("is true only when the focus is the panel terminal's own input", () => {
    expect(shellTerminalFocused({ activeElement: input(true) } as never)).toBe(true);
    // A session's terminal has the same kind of input but is not in the panel.
    expect(shellTerminalFocused({ activeElement: input(false) } as never)).toBe(false);
    // The panel's header buttons are in the panel but are not its terminal.
    expect(shellTerminalFocused({ activeElement: { classList: { contains: () => false }, closest: () => ({}) } } as never)).toBe(false);
    expect(shellTerminalFocused({ activeElement: null } as never)).toBe(false);
  });
});

describe("focusShellTerminal", () => {
  it("focuses the panel terminal's input as soon as it exists, trying again each frame until then", () => {
    const focus = vi.fn();
    let el: Fake | null = null;
    const frames: (() => void)[] = [];
    const doc = { querySelector: vi.fn((s: string) => (s === SHELL_TERMINAL_INPUT ? el : null)) };
    focusShellTerminal(doc as never, (f) => void frames.push(f), () => 0);
    expect(focus).not.toHaveBeenCalled();
    el = input(true, focus);
    frames.shift()!();
    expect(focus).toHaveBeenCalledTimes(1);
    // Done: no further frame is asked for.
    expect(frames).toHaveLength(0);
  });
  it("gives up after a few seconds rather than polling for ever", () => {
    let now = 0;
    const frames: (() => void)[] = [];
    const doc = { querySelector: () => null };
    focusShellTerminal(doc as never, (f) => void frames.push(f), () => now);
    now = 10_000;
    frames.shift()!();
    expect(frames).toHaveLength(0);
  });
  it("focuses at once when the terminal is already there", () => {
    const focus = vi.fn();
    const frames: (() => void)[] = [];
    focusShellTerminal({ querySelector: () => input(true, focus) } as never, (f) => void frames.push(f), () => 0);
    expect(focus).toHaveBeenCalledTimes(1);
    expect(frames).toHaveLength(0);
  });
});
