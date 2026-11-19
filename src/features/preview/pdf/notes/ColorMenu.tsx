import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useCoverPage } from "../../../browser/pageCover";
import { HIGHLIGHT_COLORS } from "./editor";

export type ColorMenuKey = { type: "focus"; index: number } | { type: "pick"; index: number } | { type: "close" } | null;

/** What a key does in the open colour menu: Up and Down go round, Home and End go to the ends, Enter picks the focused colour and Esc closes. */
export function colorMenuKey(key: string, index: number, count: number): ColorMenuKey {
  switch (key) {
    case "ArrowDown":
      return { type: "focus", index: (index + 1) % count };
    case "ArrowUp":
      return { type: "focus", index: (index - 1 + count) % count };
    case "Home":
      return { type: "focus", index: 0 };
    case "End":
      return { type: "focus", index: count - 1 };
    case "Enter":
      return { type: "pick", index };
    case "Escape":
      return { type: "close" };
    default:
      return null;
  }
}

type Props = {
  /** The chosen colour's value, one of `HIGHLIGHT_COLORS`. */
  value: string;
  onPick: (value: string) => void;
};

/** The highlight colour as a menu button: five radio items, the chosen one focused when it opens. */
export function ColorMenu({ value, onPick }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const chosen = Math.max(
    0,
    HIGHLIGHT_COLORS.findIndex((c) => c.value === value),
  );
  // A browser page is a webview above this one: the menu covers it while it is open.
  useCoverPage(open);

  useEffect(() => {
    if (open) items.current[chosen]?.focus({ preventScroll: true });
  }, [open, chosen]);

  // A press outside the menu, or the keyboard going elsewhere, closes it.
  useEffect(() => {
    if (!open) return;
    const down = (e: PointerEvent) => {
      if (!(e.target instanceof Node) || !root.current?.contains(e.target)) setOpen(false);
    };
    document.addEventListener("pointerdown", down, true);
    return () => document.removeEventListener("pointerdown", down, true);
  }, [open]);

  const close = (focusButton: boolean) => {
    setOpen(false);
    if (focusButton) button.current?.focus({ preventScroll: true });
  };
  const pick = (i: number) => {
    onPick(HIGHLIGHT_COLORS[i]!.value);
    close(true);
  };
  const onMenuKey = (e: KeyboardEvent) => {
    const at = items.current.findIndex((b) => b === document.activeElement);
    const k = colorMenuKey(e.key, at < 0 ? chosen : at, HIGHLIGHT_COLORS.length);
    if (!k || e.ctrlKey || e.altKey || e.metaKey) return;
    // Space picks as a button does; Esc is handled here and goes no further, so it closes the menu and not the bar.
    e.preventDefault();
    e.stopPropagation();
    if (k.type === "focus") items.current[k.index]?.focus({ preventScroll: true });
    else if (k.type === "pick") pick(k.index);
    else close(true);
  };

  return (
    <div
      ref={root}
      className="pdf-colors"
      onBlur={(e) => {
        if (open && !(e.relatedTarget instanceof Node && root.current?.contains(e.relatedTarget))) setOpen(false);
      }}
    >
      <button
        ref={button}
        type="button"
        className="pdf-selbar__button pdf-colors__button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`Highlight colour: ${HIGHLIGHT_COLORS[chosen]!.name}`}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && !open) {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        <span className="pdf-colors__swatch" style={{ background: HIGHLIGHT_COLORS[chosen]!.value }} aria-hidden="true" />
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
          <path d="M2 3.5 5 6.5 8 3.5" />
        </svg>
      </button>
      {open && (
        <div className="pdf-colors__menu" role="menu" aria-label="Highlight colour" onKeyDown={onMenuKey}>
          {HIGHLIGHT_COLORS.map((c, i) => (
            <button
              key={c.name}
              ref={(el) => {
                items.current[i] = el;
              }}
              type="button"
              role="menuitemradio"
              aria-checked={i === chosen}
              tabIndex={i === chosen ? 0 : -1}
              className="pdf-colors__item"
              onClick={() => pick(i)}
            >
              <span className="pdf-colors__swatch" style={{ background: c.value }} aria-hidden="true" />
              {c.name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
