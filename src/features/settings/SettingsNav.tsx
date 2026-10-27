import { useEffect, useRef } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";

/** The settings sections as a nav: one button each, the current one marked with the shared selected mark. */
export function SettingsNav<Id extends string>({
  items,
  current,
  onPick,
  className = "settings__nav",
  ariaCurrent = "true",
}: {
  items: ReadonlyArray<{ id: Id; label: string }>;
  current: Id;
  onPick?(id: Id): void;
  className?: string;
  ariaCurrent?: "true" | "page";
}) {
  const nav = useRef<HTMLElement>(null);
  // The nav scrolls on its own when it is taller than the page: keep the current section in view after a change.
  useEffect(() => {
    nav.current?.querySelector<HTMLElement>('[aria-current]')?.scrollIntoView?.({ block: "nearest" });
  }, [current]);
  return (
    <nav ref={nav} aria-label="Settings sections" className={className}>
      <ul>
        {items.map((n) => {
          const props = selectedProps(n.id === current, "current", "settings__nav-item");
          return (
            <li key={n.id}>
              {/* SelectedMark.css keys on aria-current="true", so "page" also needs data-selected to draw the mark. */}
              <button type="button" {...props} {...(props["aria-current"] ? { "aria-current": ariaCurrent, "data-selected": ariaCurrent === "page" ? "true" : undefined } : {})} onClick={onPick ? () => onPick(n.id) : undefined}>
                {n.label}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
