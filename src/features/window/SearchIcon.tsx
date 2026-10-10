/** A magnifier: the one symbol everyone reads as "search", used for the command palette. */
export function SearchIcon({ size = 14 }: { size?: number }) {
  return (
    <svg className="search-icon" data-icon="search" width={size} height={size} viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="6" cy="6" r="4" />
      <path d="M9 9l3.5 3.5" />
    </svg>
  );
}
