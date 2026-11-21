import { describe, expect, it } from "vitest";
import { relayFrameFocus } from "./frameFocus";

describe("relayFrameFocus", () => {
  it("tells the app's document when the keyboard goes into the frame and when it leaves", () => {
    const frameWindow = new EventTarget();
    const element = new EventTarget();
    const seen: [string, boolean][] = [];
    for (const type of ["focusin", "focusout"]) element.addEventListener(type, (e) => seen.push([e.type, e.bubbles]));
    relayFrameFocus(frameWindow, element);
    frameWindow.dispatchEvent(new Event("focus"));
    expect(seen).toEqual([["focusin", true]]);
    frameWindow.dispatchEvent(new Event("blur"));
    expect(seen).toEqual([
      ["focusin", true],
      ["focusout", true],
    ]);
  });
  it("says nothing before the frame's window says something", () => {
    const element = new EventTarget();
    let heard = 0;
    element.addEventListener("focusin", () => heard++);
    relayFrameFocus(new EventTarget(), element);
    expect(heard).toBe(0);
  });
});
