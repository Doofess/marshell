/**
 * A rolled Ctrl+Shift+Tab (docs/PLAN.md §9, "Slice 6q-3 decisions" 14 and 15). Ctrl+Tab ("Next tab") is the navigation chord
 * Ctrl+Shift+Tab without its Shift, and a quick hand can put Tab down a few milliseconds before Shift. So a fresh Ctrl+Tab is held
 * until Tab comes up or TAB_HOLD_MS pass, and goes on as Ctrl+Shift+Tab if Shift went down with Ctrl meanwhile.
 */
export const TAB_HOLD_MS = 50;

export type HoldKey = { type: string; code: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean; repeat: boolean };
/** `release` sends the held chord now (`shift`: as Ctrl+Shift+Tab); `swallow` keeps this key from every other handler. */
export type HoldStep = { held: boolean; release: { shift: boolean } | null; swallow: boolean };

const SHIFT = /^Shift(Left|Right)$/;

/** Only Ctrl and Shift, with no Alt or Meta, make the navigation chord. */
const chordShift = (e: HoldKey): boolean => e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey;

/** What a key does to a Ctrl+Tab that may be held. */
export function tabHoldStep(held: boolean, e: HoldKey): HoldStep {
  if (!held) {
    const fresh = e.type === "keydown" && e.code === "Tab" && e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && !e.repeat;
    return { held: fresh, release: null, swallow: fresh };
  }
  if (e.type === "keyup") {
    // Ctrl coming up first changes nothing yet; the timer ends the hold.
    return e.code === "Tab" ? { held: false, release: { shift: chordShift(e) }, swallow: false } : { held: true, release: null, swallow: false };
  }
  if (e.code === "Tab" && e.repeat) return { held: true, release: null, swallow: true };
  // Shift turns the held key into Ctrl+Shift+Tab; any other key ends the hold as Ctrl+Tab, and then goes on itself.
  return { held: false, release: { shift: SHIFT.test(e.code) && chordShift(e) }, swallow: false };
}

export type HoldWindow = Pick<Window, "addEventListener" | "removeEventListener">;

/**
 * Holds each real Ctrl+Tab on `win`, ahead of every other handler (the capture phase of the window), and gives the settled chord to
 * `send` with the element the key was pressed on. Only real presses are held: a page's forwarded key was settled by the shell, and
 * the re-sent key is synthetic too. A blur drops a hold unsent, since the person has moved on. Returns the uninstall.
 */
export function installTabHold(win: HoldWindow, send: (target: EventTarget | null, shift: boolean) => void): () => void {
  let held = false;
  let target: EventTarget | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const end = () => {
    held = false;
    target = null;
    clearTimeout(timer);
  };
  const onKey = (e: KeyboardEvent) => {
    if (!e.isTrusted) return;
    const step = tabHoldStep(held, e);
    if (step.release) {
      const to = target;
      end();
      send(to, step.release.shift);
    } else if (step.held && !held) {
      held = true;
      target = e.target;
      timer = setTimeout(() => {
        const to = target;
        end();
        send(to, false);
      }, TAB_HOLD_MS);
    }
    if (step.swallow) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  };
  win.addEventListener("keydown", onKey, true);
  win.addEventListener("keyup", onKey, true);
  win.addEventListener("blur", end);
  return () => {
    end();
    win.removeEventListener("keydown", onKey, true);
    win.removeEventListener("keyup", onKey, true);
    win.removeEventListener("blur", end);
  };
}

/** Re-sends a settled Ctrl+Tab or Ctrl+Shift+Tab where it was pressed, or where the keyboard is when that element has gone. */
export function sendHeldTab(target: EventTarget | null, shift: boolean): void {
  const to = target instanceof Node && target.isConnected ? target : (document.activeElement ?? document.body);
  to.dispatchEvent(new KeyboardEvent("keydown", { code: "Tab", key: "Tab", ctrlKey: true, shiftKey: shift, bubbles: true, cancelable: true }));
}
