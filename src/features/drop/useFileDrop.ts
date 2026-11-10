import { useEffect, useRef } from "react";
import { inTauri } from "../../lib/window/appWindow";
import { onBrowser } from "../browser/browserApi";
import { coverPage } from "../browser/pageCover";
import { createDragCover } from "./dragCover";
import { dropTarget, pageDrop, toCss, type DropTarget } from "./dropTarget";
import "./Drop.css";

type Handlers = {
  onPaths: (target: DropTarget, paths: string[]) => void;
  onHover: (target: DropTarget | null) => void;
};

const targetAt = (position: { x: number; y: number }): DropTarget => {
  const { x, y } = toCss(position, window.devicePixelRatio);
  return dropTarget(document.elementFromPoint(x, y));
};

/** Files dragged in from the system (Tauri's native drop). Outside Tauri (Storybook, tests) it does nothing. */
export function useFileDrop(handlers: Handlers): void {
  const ref = useRef(handlers);
  ref.current = handlers;
  useEffect(() => {
    if (!inTauri()) return;
    let off: (() => void) | undefined;
    let gone = false;
    // A drag covers the page from its enter to its leave or drop, so the page freezes as a picture and the drag reaches this webview.
    const cover = createDragCover(coverPage, (fn, ms) => {
      const timer = setTimeout(fn, ms);
      return () => clearTimeout(timer);
    });
    // A drag that enters over a page is heard by the page's webview first; once the page is hidden this one hears the rest.
    const offPage = onBrowser("drag", (d) => {
      if (d.kind === "enter") cover.pageEnter();
      else if (d.kind === "leave") cover.pageLeave();
      else cover.pageDrop();
      // A file dropped on a page is a drop on the pane: its files open as tabs, as a drop on any other part of the pane does.
      const dropped = pageDrop(d);
      if (dropped) ref.current.onPaths(dropped.target, dropped.paths);
    });
    void import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === "enter") cover.appEnter();
          if (p.type === "enter" || p.type === "over") ref.current.onHover(targetAt(p.position));
          else if (p.type === "leave") {
            cover.appEnd();
            ref.current.onHover(null);
          } else if (p.type === "drop") {
            cover.appEnd();
            ref.current.onHover(null);
            if (p.paths.length > 0) ref.current.onPaths(targetAt(p.position), p.paths);
          }
        })
        .then((un) => {
          if (gone) un();
          else off = un;
        }),
    );
    return () => {
      gone = true;
      off?.();
      offPage();
      cover.dispose();
    };
  }, []);
}
