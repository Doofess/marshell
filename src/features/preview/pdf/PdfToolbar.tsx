import type { ReactNode } from "react";

type Props = {
  page: string;
  count: number;
  percent: number;
  fit: boolean;
  findOpen: boolean;
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
        <button type="button" className="pdf__button pdf__icon" disabled={off} onClick={p.onZoomOut} aria-label="Zoom out" title="Zoom out (Ctrl+-)">
          {icon("M3 7h8")}
        </button>
        <span className="pdf__percent">{p.percent}%</span>
        <button type="button" className="pdf__button pdf__icon" disabled={off} onClick={p.onZoomIn} aria-label="Zoom in" title="Zoom in (Ctrl+=)">
          {icon("M3 7h8", "M7 3v8")}
        </button>
        <button type="button" className="pdf__button pdf__icon" disabled={off} aria-pressed={p.fit} onClick={p.onFit} aria-label="Fit width" title="Fit width (Ctrl+0)">
          {icon("M1.5 7h11", "M4 4.5 1.5 7 4 9.5", "M10 4.5 12.5 7 10 9.5")}
        </button>
        <button type="button" className="pdf__button pdf__icon" disabled={off} aria-expanded={p.findOpen} aria-keyshortcuts="Control+F" onClick={p.onFind} aria-label="Find" title="Find (Ctrl+F)">
          {icon("M6 10.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9Z", "M9.3 9.3 12.5 12.5")}
        </button>
      </div>
      {p.findRow}
    </>
  );
}
