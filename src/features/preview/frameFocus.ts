/**
 * A rendered HTML file is a frame with a document of its own. When the keyboard goes into it, the app's document hears nothing:
 * no `focusin` reaches it, though `document.activeElement` becomes the frame's element. So the frame's window says it here, as a
 * `focusin` (and a `focusout` when the keyboard leaves) sent from the frame's element, which the app's own listeners then read
 * as they read any other: the window knows which panel has the keyboard, and the pane knows it is inside.
 */
export function relayFrameFocus(frameWindow: EventTarget, element: EventTarget): void {
  frameWindow.addEventListener("focus", () => element.dispatchEvent(new Event("focusin", { bubbles: true })));
  frameWindow.addEventListener("blur", () => element.dispatchEvent(new Event("focusout", { bubbles: true })));
}
