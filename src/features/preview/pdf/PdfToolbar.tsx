import { useId, type ReactNode } from "react";
import type { NoteTool } from "./notes/editor";

type Props = {
  page: string;
  count: number;
  percent: number;
  fit: boolean;
  findOpen: boolean;
  /** The note tool on: pressing the pressed one turns it off. */
  tool: NoteTool;
  /** Why notes can't be added, or null; the two tools are then disabled and the reason is their title. */
  blocked: string | null;
  onTool: (t: NoteTool) => void;
  /** The sidebar button's slot, first in the row. */
  sidebar: ReactNode;
  onPageDraft: (v: string) => void;
  onPageCommit: () => void;
  onPageCancel: () => void;
  onZoomOut: () => void;
  onZoomIn: () => void;
  onFit: () => void;
  onFind: () => void;
  /** The find row, under the buttons. */
  findRow: ReactNode;
};

const icon = (...d: string[]) => (
  <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    {d.map((p) => (
      <path key={p} d={p} />
    ))}
  </svg>
);

/** The toolbar of a PDF tab: icon buttons, the Page field and the percentage, with the find row below. */
export function PdfToolbar(p: Props) {
  const off = p.count === 0;
  const percentId = useId();
  // Blocked tools are aria-disabled, not disabled: they keep their place in the tab order and say why they do nothing.
  const blocked = p.blocked !== null || undefined;
  const toolTitle = (title: string) => (p.blocked !== null ? `Notes can't be added: ${p.blocked}` : title);
  return (
    <>
      <div className="pdf__row">
        {p.sidebar}
        <label className="pdf__pages">
          <span className="pdf__word">Page</span>
          <input
            className="pdf__field"
            inputMode="numeric"
            autoComplete="off"
            spellCheck={false}
            disabled={off}
            value={p.page}
            aria-label={`Page, of ${p.count}`}
            onFocus={(e) => e.currentTarget.select()}
            onChange={(e) => p.onPageDraft(e.target.value)}
            onBlur={p.onPageCancel}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                p.onPageCommit();
              }
            }}
          />
          <span>of {p.count}</span>
        </label>
        <span className="pdf__spacer" />
        <button type="button" className="pdf__button pdf__icon" disabled={off} onClick={p.onZoomOut} aria-describedby={percentId} aria-label="Zoom out" title="Zoom out (Ctrl+-)">
          {icon("M3 7h8")}
        </button>
        <span className="pdf__percent" id={percentId}>{p.percent}%</span>
        <button type="button" className="pdf__button pdf__icon" disabled={off} onClick={p.onZoomIn} aria-describedby={percentId} aria-label="Zoom in" title="Zoom in (Ctrl+=)">
          {icon("M3 7h8", "M7 3v8")}
        </button>
        <button type="button" className="pdf__button pdf__icon" disabled={off} aria-pressed={p.fit} onClick={p.onFit} aria-label="Fit width" title="Fit width (Ctrl+0)">
          {icon("M1.5 7h11", "M4 4.5 1.5 7 4 9.5", "M10 4.5 12.5 7 10 9.5")}
        </button>
        {/* A marker: its tip at the lower left, a stroke of colour under it. */}
        <button type="button" className="pdf__button pdf__icon" disabled={off} aria-disabled={blocked} aria-pressed={p.tool === "highlight"} onClick={blocked ? undefined : () => p.onTool(p.tool === "highlight" ? "none" : "highlight")} aria-label="Highlight" title={toolTitle("Highlight: drag across text")}>
          {icon("M8.5 2.5 11.5 5.5 6 11H3.5V8.5Z", "M7 4l3 3", "M2 12.5h10")}
        </button>
        <button type="button" className="pdf__button pdf__icon" disabled={off} aria-disabled={blocked} aria-pressed={p.tool === "freetext"} onClick={blocked ? undefined : () => p.onTool(p.tool === "freetext" ? "none" : "freetext")} aria-label="Free text" title={toolTitle("Free text: click where it goes, or press Enter")}>
          {icon("M3 3.5V2.5h8v1", "M7 2.5v9", "M5.5 11.5h3")}
        </button>
        <button type="button" className="pdf__button pdf__icon" disabled={off} aria-expanded={p.findOpen} aria-keyshortcuts="Control+F" onClick={p.onFind} aria-label="Find" title="Find (Ctrl+F)">
          {icon("M6 10.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9Z", "M9.3 9.3 12.5 12.5")}
        </button>
      </div>
      {p.findRow}
    </>
  );
}
