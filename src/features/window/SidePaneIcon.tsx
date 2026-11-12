/** The side pane button's icon: a window with its end panel marked, the mirror of SidebarIcon. The panel is filled while the pane is open. */
export function SidePaneIcon({ open }: { open: boolean }) {
  return (
    <svg className="side-pane-icon" data-icon="side-pane" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round">
      {open && <path d="M10 2.5h2a2.5 2.5 0 0 1 2.5 2.5v6a2.5 2.5 0 0 1-2.5 2.5h-2z" fill="currentColor" fillOpacity="0.35" stroke="none" />}
      <rect x="1.5" y="2.5" width="13" height="11" rx="2.5" />
      <path d="M10 2.5v11" />
    </svg>
  );
}
