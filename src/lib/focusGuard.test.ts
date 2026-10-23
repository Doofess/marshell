import { describe, expect, it } from "vitest";
import { holdFocus, tabbable, type GuardDom, type GuardNode } from "./focusGuard";

type Node = GuardNode & { name: string; focus(): void };

/** A page reduced to what the guard touches: popups that contain nodes, who has focus, the three listeners and a frame queue. */
function page() {
  let active: Node | null = null;
  const log: string[] = [];
  const parent = new Map<Node, Node>();
  const layers = new Set<Node>();
  const node = (name: string, inside?: Node): Node => {
    const n: Node = {
      name,
      isConnected: true,
      contains: (x) => {
        for (let c = x as Node | undefined; c; c = parent.get(c)) if (c === n) return true;
        return false;
      },
      focus: () => {
        log.push(name);
        active = n;
      },
    };
    if (inside) parent.set(n, inside);
    return n;
  };
  const focusIn: ((t: unknown) => void)[] = [];
  const focusOut = new Map<Node, (r: unknown) => void>();
  const winFocus: (() => void)[] = [];
  let frames: (() => void)[] = [];
  const dom = (popup: Node): GuardDom => ({
    active: () => active,
    inOtherLayer: (t) => [...layers].some((l) => l !== popup && !l.contains(popup) && l.contains(t)),
    onFocusIn: (h) => (focusIn.push(h), () => void focusIn.splice(focusIn.indexOf(h), 1)),
    onFocusOut: (h) => (focusOut.set(popup, h), () => void focusOut.delete(popup)),
    onWindowFocus: (h) => (winFocus.push(h), () => void winFocus.splice(winFocus.indexOf(h), 1)),
    later: (f) => {
      frames.push(f);
      return () => void (frames = frames.filter((x) => x !== f));
    },
  });
  /** Focus moves to `to` (null: nothing, as when the window loses focus); the listeners hear it as the browser would tell them. */
  const move = (to: Node | null) => {
    const from = active;
    active = to;
    for (const [p, h] of focusOut) if (from && p.contains(from) && !(to && p.contains(to))) h(to);
    if (to) for (const h of [...focusIn]) h(to);
  };
  const frame = () => {
    const run = frames;
    frames = [];
    run.forEach((f) => f());
  };
  return { node, dom, move, frame, log, layers, winFocus: () => winFocus.forEach((h) => h()), active: () => active, pending: () => frames.length };
}

describe("holdFocus: an open popup keeps the focus", () => {
  it("takes the focus back, a frame later, when something outside it is focused", () => {
    const p = page();
    const menu = p.node("menu");
    const item = p.node("item", menu);
    const terminal = p.node("terminal");
    item.focus();
    const stop = holdFocus(menu, p.dom(menu), () => null);
    p.move(item);
    p.move(terminal);
    expect(p.active()).toBe(terminal);
    p.frame();
    expect(p.active()).toBe(item);
    stop();
  });

  it("takes it back when focus falls to nothing inside the page", () => {
    const p = page();
    const sheet = p.node("sheet");
    const prompt = p.node("prompt", sheet);
    const stop = holdFocus(sheet, p.dom(sheet), () => prompt);
    p.move(prompt);
    p.move(null);
    p.frame();
    expect(p.active()).toBe(prompt);
    stop();
  });

  it("takes it back when the window comes back with nothing focused", () => {
    const p = page();
    const sheet = p.node("sheet");
    const prompt = p.node("prompt", sheet);
    const stop = holdFocus(sheet, p.dom(sheet), () => prompt);
    p.move(prompt);
    p.move(null);
    p.frame();
    p.log.length = 0;
    p.move(null);
    p.winFocus();
    p.frame();
    expect(p.log).toEqual(["prompt"]);
    stop();
  });

  it("does nothing when the window comes back with the focus still inside", () => {
    const p = page();
    const sheet = p.node("sheet");
    const prompt = p.node("prompt", sheet);
    const stop = holdFocus(sheet, p.dom(sheet), () => prompt);
    p.move(prompt);
    p.log.length = 0;
    p.winFocus();
    p.frame();
    expect(p.log).toEqual([]);
    stop();
  });

  it("returns to the last control focused inside, not the fallback", () => {
    const p = page();
    const sheet = p.node("sheet");
    const prompt = p.node("prompt", sheet);
    const start = p.node("start", sheet);
    const stop = holdFocus(sheet, p.dom(sheet), () => prompt);
    p.move(start);
    p.move(p.node("row"));
    p.frame();
    expect(p.active()).toBe(start);
    stop();
  });

  it("uses the fallback when the last control has gone from the page", () => {
    const p = page();
    const sheet = p.node("sheet");
    const prompt = p.node("prompt", sheet);
    const chip = p.node("chip", sheet);
    const stop = holdFocus(sheet, p.dom(sheet), () => prompt);
    p.move(chip);
    chip.isConnected = false;
    p.move(null);
    p.frame();
    expect(p.active()).toBe(prompt);
    stop();
  });

  it("leaves the focus alone when it moves into another dialog or menu", () => {
    const p = page();
    const sheet = p.node("sheet");
    const prompt = p.node("prompt", sheet);
    const other = p.node("other dialog");
    const title = p.node("title", other);
    p.layers.add(sheet).add(other);
    const stop = holdFocus(sheet, p.dom(sheet), () => prompt);
    p.move(prompt);
    p.move(title);
    p.frame();
    expect(p.active()).toBe(title);
    stop();
  });

  it("acts only as the topmost guard: the newer popup owns the focus while it is open", () => {
    const p = page();
    const dialog = p.node("dialog");
    const ok = p.node("ok", dialog);
    const menu = p.node("menu");
    const item = p.node("item", menu);
    const terminal = p.node("terminal");
    const stopDialog = holdFocus(dialog, p.dom(dialog), () => ok);
    p.move(ok);
    const stopMenu = holdFocus(menu, p.dom(menu), () => item);
    p.move(item);
    p.move(terminal);
    p.frame();
    expect(p.active()).toBe(item);
    stopMenu();
    p.move(terminal);
    p.frame();
    expect(p.active()).toBe(ok);
    stopDialog();
  });

  it("does nothing once stopped, even with a frame already queued", () => {
    const p = page();
    const menu = p.node("menu");
    const item = p.node("item", menu);
    const terminal = p.node("terminal");
    const stop = holdFocus(menu, p.dom(menu), () => item);
    p.move(item);
    p.move(terminal);
    stop();
    p.frame();
    expect(p.active()).toBe(terminal);
    expect(p.pending()).toBe(0);
  });

  it("does nothing when the popup left the page before the frame (a close that moves the focus on)", () => {
    const p = page();
    const menu = p.node("menu");
    const item = p.node("item", menu);
    const box = p.node("rename box");
    const stop = holdFocus(menu, p.dom(menu), () => item);
    p.move(item);
    p.move(box);
    menu.isConnected = false;
    p.frame();
    expect(p.active()).toBe(box);
    stop();
  });
});

describe("tabbable: the controls a popup's Tab walks", () => {
  const el = (tabIndex: number, shown = true) => ({ tabIndex, shown });
  it("keeps the shown ones in the tab order, and drops roving and hidden ones", () => {
    const a = el(0);
    const b = el(-1);
    const c = el(0, false);
    const d = el(0);
    expect(tabbable([a, b, c, d], (x) => x.shown)).toEqual([a, d]);
  });
});
