/** A side pane panel button's icon: a globe for Browser, a folder for Files, a page with a plus and a minus for Changes, and a folder for any other id. The shape is tinted while the panel is open. */
export function PanelIcon({ panel, open }: { panel: string; open: boolean }) {
  const fill = open ? { fill: "currentColor", fillOpacity: 0.35 } : {};
  return (
    <svg className="panel-icon" data-icon={`panel-${panel === "browser" ? "browser" : panel === "changes" ? "changes" : "files"}`} width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
      {panel === "browser" ? (
        <>
          <circle cx="8" cy="8" r="6" {...fill} />
          <path d="M2 8h12" />
          <path d="M8 2c1.8 1.7 2.7 3.7 2.7 6S9.8 12.3 8 14C6.2 12.3 5.3 10.3 5.3 8S6.2 3.7 8 2z" />
        </>
      ) : panel === "changes" ? (
        <>
          <rect x="2" y="2" width="12" height="12" rx="2" {...fill} />
          <path d="M8 4.5v4M6 6.5h4M6 11h4" />
        </>
      ) : (
        <path d="M1.5 4.5a1.5 1.5 0 0 1 1.5-1.5h3l1.5 1.5H13a1.5 1.5 0 0 1 1.5 1.5v6a1.5 1.5 0 0 1-1.5 1.5H3a1.5 1.5 0 0 1-1.5-1.5z" {...fill} />
      )}
    </svg>
  );
}
