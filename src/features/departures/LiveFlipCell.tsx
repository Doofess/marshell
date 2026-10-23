import { useEffect, useRef, useState } from "react";
import { flipDuration, flipFrame, retarget, type FlipRun } from "./flip";

export function LiveFlipCell({ text, reduced }: { text: string; reduced: boolean }) {
  const cell = useRef<HTMLSpanElement>(null);
  const last = useRef(text);
  const shown = useRef(text);
  const run = useRef<FlipRun | null>(null);
  const [frame, setFrame] = useState(text);

  useEffect(() => {
    const changed = last.current !== text;
    last.current = text;
    if (reduced) {
      run.current = null;
      shown.current = text;
      setFrame(text);
      const node = cell.current;
      if (!changed || !node) return;
      // Flush the removed animation so a second change restarts even during a fade.
      node.removeAttribute("data-fade");
      void node.offsetWidth;
      node.setAttribute("data-fade", "true");
      const timer = setTimeout(() => node.removeAttribute("data-fade"), 200);
      return () => {
        clearTimeout(timer);
        node.removeAttribute("data-fade");
      };
    }

    const now = performance.now();
    const next = retarget(run.current, shown.current, text, now);
    run.current = next;
    if (!next) return;
    let frameId: number | undefined;
    const tick = (time: number) => {
      const elapsed = time - next.start;
      setFrame(flipFrame(next.from, next.to, elapsed));
      if (elapsed >= flipDuration(Math.max(next.from.length, next.to.length))) {
        shown.current = next.to;
        run.current = null;
      } else {
        frameId = requestAnimationFrame(tick);
      }
    };
    tick(now);
    return () => {
      if (frameId !== undefined) cancelAnimationFrame(frameId);
    };
  }, [text, reduced]);

  return (
    <span ref={cell} className="flip" data-live>
      <span aria-hidden="true">{reduced ? text : frame}</span>
      <span className="board__sr">{text}</span>
    </span>
  );
}
