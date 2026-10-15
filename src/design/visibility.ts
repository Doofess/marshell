/** Just the parts of Document this needs, so tests can pass a small fake. */
type Doc = {
  visibilityState: DocumentVisibilityState;
  documentElement: Pick<HTMLElement, "setAttribute" | "removeAttribute">;
  addEventListener(type: "visibilitychange", listener: () => void): void;
  removeEventListener(type: "visibilitychange", listener: () => void): void;
};

/** Mirrors page visibility onto `data-page-hidden` so CSS can pause looping animation. */
export function syncPageHidden(doc: Doc): () => void {
  const apply = () => {
    if (doc.visibilityState === "hidden") doc.documentElement.setAttribute("data-page-hidden", "");
    else doc.documentElement.removeAttribute("data-page-hidden");
  };
  apply();
  doc.addEventListener("visibilitychange", apply);
  return () => doc.removeEventListener("visibilitychange", apply);
}
