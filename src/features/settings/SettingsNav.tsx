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
  return (
    <nav aria-label="Settings sections" className={className}>
      <ul>
        {items.map((n) => {
          const props = selectedProps(n.id === current, "current", "settings__nav-item");
          return (
            <li key={n.id}>
              <button type="button" {...props} {...(props["aria-current"] ? { "aria-current": ariaCurrent } : {})} onClick={onPick ? () => onPick(n.id) : undefined}>
                {n.label}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
