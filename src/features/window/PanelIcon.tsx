/** A side pane panel button's icon: a globe for Browser, a folder for Files and for any other id. The shape is tinted while the panel is open. */
export function PanelIcon({ panel, open }: { panel: string; open: boolean }) {
  const fill = open ? { fill: "currentColor", fillOpacity: 0.35 } : {};
  return (
    <svg className="panel-icon" data-icon={`panel-${panel === "browser" ? "browser" : "files"}`} width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
      {panel === "browser" ? (
        <>
          <circle cx="8" cy="8" r="6" {...fill} />
          <path d="M2 8h12" />
          <path d="M8 2c1.8 1.7 2.7 3.7 2.7 6S9.8 12.3 8 14C6.2 12.3 5.3 10.3 5.3 8S6.2 3.7 8 2z" />
        </>
      ) : (
        <path d="M1.5 4.5a1.5 1.5 0 0 1 1.5-1.5h3l1.5 1.5H13a1.5 1.5 0 0 1 1.5 1.5v6a1.5 1.5 0 0 1-1.5 1.5H3a1.5 1.5 0 0 1-1.5-1.5z" {...fill} />
      )}
    </svg>
  );
}
