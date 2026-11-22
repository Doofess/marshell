import "./FilesBusy.css";

/** The ring and its line, in the middle of the tree area while Files waits. The ring is decoration: the area's `aria-busy` says it. */
export function FilesBusy(p: { text: string }) {
  return (
    <>
      <svg className="files-busy__ring" width="40" height="40" viewBox="0 0 40 40" aria-hidden="true" focusable="false">
        <circle className="files-busy__track" cx="20" cy="20" r="16" fill="none" stroke="currentColor" strokeWidth="3" />
        <path className="files-busy__arc" d="M 20 4 A 16 16 0 0 1 36 20" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
      </svg>
      <p className="files-busy__text">{p.text}</p>
    </>
  );
}
