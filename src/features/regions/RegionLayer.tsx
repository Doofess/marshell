import { forwardRef, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { describeRegion, nudge, type Region } from "./region";
import "./Regions.css";

type Props = {
  region: Region;
  onChange: (r: Region) => void;
  onConfirm: () => void;
  onCancel: () => void;
};

const ARROWS = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"] as const;

/** The box over a page: focusable, moved and resized with the arrow keys. The page wrapper it sits in is positioned. */
export const RegionLayer = forwardRef<HTMLDivElement, Props>(function RegionLayer({ region, onChange, onConfirm, onCancel }, ref) {
  const own = useRef<HTMLDivElement | null>(null);
  // What a screen reader hears follows the last move by 300 ms, so a run of key presses is one sentence.
  const [spoken, setSpoken] = useState(() => describeRegion(region));
  useEffect(() => {
    const t = setTimeout(() => setSpoken(describeRegion(region)), 300);
    return () => clearTimeout(t);
  }, [region]);

  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.altKey || e.metaKey) return;
    const page = own.current?.parentElement;
    if ((ARROWS as readonly string[]).includes(e.key) && page) {
      e.preventDefault();
      e.stopPropagation();
      const size = { w: page.clientWidth || 1, h: page.clientHeight || 1 };
      onChange({ ...nudge(region, e.key as (typeof ARROWS)[number], { shift: e.shiftKey, ctrl: e.ctrlKey }, size), page: region.page });
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      onConfirm();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    }
  };

  return (
    <div
      ref={(el) => {
        own.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      className="region-box"
      tabIndex={0}
      role="group"
      aria-roledescription="region"
      aria-label={describeRegion(region)}
      style={{ left: `${region.x * 100}%`, top: `${region.y * 100}%`, width: `${region.w * 100}%`, height: `${region.h * 100}%` }}
      onKeyDown={keys}
      onDoubleClick={onConfirm}
    >
      <span className="region-box__live" role="status" aria-live="polite">
        {spoken}
      </span>
    </div>
  );
});
