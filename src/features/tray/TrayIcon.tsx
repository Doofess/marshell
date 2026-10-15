/**
 * The tray icon: the mono marshalling wands (assets/icon/tray-mono.svg) in the current colour, so the OS can tint it for a
 * light or a dark panel, with an accent dot when something waits. The wrapper carries the name; the drawing is hidden.
 */
export function TrayIcon({ size, waiting = 0 }: { size: 16 | 32; waiting?: number }) {
  const label = waiting > 0 ? `Marshell, ${waiting} ${waiting === 1 ? "needs" : "need"} you` : "Marshell";
  return (
    <span className="tray-icon" role="img" aria-label={label}>
      <svg aria-hidden="true" width={size} height={size} viewBox="0 0 1024 1024">
        <g stroke="currentColor" strokeWidth="170" strokeLinecap="round">
          <line x1="400" y1="884" x2="168" y2="150" />
          <line x1="624" y1="884" x2="856" y2="150" />
        </g>
        {waiting > 0 && <circle cx="830" cy="190" r="170" fill="var(--accent)" />}
      </svg>
    </span>
  );
}
