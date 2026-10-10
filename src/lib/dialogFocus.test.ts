import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { openDialogFocus, type DialogDom, type Focusable } from "./dialogFocus";

/** A dialog's DOM reduced to what the lifecycle touches: who has focus, the title, the controls and one keydown listener. */
function fakeDialog(names: string[]) {
  const log: string[] = [];
  let active: Focusable | null = null;
  const node = (name: string): Focusable => {
    const n: Focusable = {
      focus: () => {
        log.push(name);
        active = n;
      },
    };
    return n;
  };
  const opener = node("opener");
  const title = node("title");
  const controls = names.map(node);
  active = opener;
  let handler: ((e: { key: string; shiftKey?: boolean; preventDefault(): void }) => void) | null = null;
  let prevented = 0;
  const dom: DialogDom = {
    active: () => active,
    title,
    controls: () => controls,
    onKeydown: (h) => {
      handler = h;
      return () => {
        handler = null;
      };
    },
  };
  const press = (key: string, shiftKey = false) => handler?.({ key, shiftKey, preventDefault: () => prevented++ });
  return { dom, log, press, prevented: () => prevented, listening: () => handler !== null };
}

describe("openDialogFocus: one run per open", () => {
  it("remembers the opener and moves focus to the title, once", () => {
    const d = fakeDialog(["Cancel", "Fix"]);
    openDialogFocus(d.dom, () => {});
    expect(d.log).toEqual(["title"]);
  });
  it("Tab and Shift+Tab walk the controls and wrap, so focus never leaves", () => {
    const d = fakeDialog(["body", "Cancel", "Fix"]);
    openDialogFocus(d.dom, () => {});
    for (let i = 0; i < 4; i++) d.press("Tab");
    expect(d.log).toEqual(["title", "body", "Cancel", "Fix", "body"]);
    d.press("Tab", true);
    expect(d.log.at(-1)).toBe("Fix");
    expect(d.prevented()).toBe(5);
  });
  it("Escape closes through the callback as it is at that moment, and other keys reach the control", () => {
    const d = fakeDialog(["Cancel"]);
    const closed: string[] = [];
    let onClose = () => closed.push("first");
    openDialogFocus(d.dom, () => onClose());
    d.press("Escape");
    // A re-render handed the dialog a new callback: nothing re-ran, and the new one is the one that closes.
    onClose = () => closed.push("second");
    d.press("Escape");
    d.press("a");
    expect(closed).toEqual(["first", "second"]);
    expect(d.prevented()).toBe(2);
    expect(d.log).toEqual(["title"]);
  });
  it("ending it stops listening and puts focus back on the opener", () => {
    const d = fakeDialog(["Cancel"]);
    const end = openDialogFocus(d.dom, () => {});
    end();
    expect(d.listening()).toBe(false);
    expect(d.log).toEqual(["title", "opener"]);
    d.press("Escape");
    expect(d.log).toEqual(["title", "opener"]);
  });
});

describe("useDialogFocus runs that lifecycle once per open", () => {
  // There is no DOM in this suite, so the hook itself is pinned at the source: the effect depends on the refs only and reads
  // onClose through an effect event. With onClose in the dependencies, every re-render (the doctor's "Checked N s ago" tick)
  // tore the dialog down and set it up again, which refocused the opener and then the title.
  it("does not list onClose among the effect's dependencies", () => {
    const src = readFileSync(new URL("./useDialogFocus.ts", import.meta.url), "utf8");
    expect(src).toMatch(/\[dialog, title\]\);/);
    expect(src).not.toMatch(/\[dialog, title, onClose\]/);
    expect(src).toContain("useEffectEvent");
    expect(src).toContain("openDialogFocus(");
  });
});
