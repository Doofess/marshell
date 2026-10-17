/** The sidebar button's icon: a window with its left panel marked. The panel is filled while the sidebar is open. */
export function SidebarIcon({ open }: { open: boolean }) {
  return (
    <svg className="sidebar-icon" data-icon="sidebar" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round">
      {open && <path d="M6 2.5H4A2.5 2.5 0 0 0 1.5 5v6A2.5 2.5 0 0 0 4 13.5h2z" fill="currentColor" fillOpacity="0.35" stroke="none" />}
      <rect x="1.5" y="2.5" width="13" height="11" rx="2.5" />
      <path d="M6 2.5v11" />
    </svg>
  );
}
