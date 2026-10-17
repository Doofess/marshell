import { useEffect, useRef } from "react";
import { inTauri } from "../../lib/window/appWindow";
import { dropTarget, toCss, type DropTarget } from "./dropTarget";
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
    void import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === "enter" || p.type === "over") ref.current.onHover(targetAt(p.position));
          else if (p.type === "leave") ref.current.onHover(null);
          else if (p.type === "drop") {
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
    };
  }, []);
}
