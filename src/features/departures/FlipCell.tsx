import { flipFrames } from "./flip";

/**
 * A cell of the board that flips like a split-flap tile when its text changes. The server markup is the FINAL text, so
 * it reads right without script, in print and for screen readers; the review page's script plays the precomputed frames.
 * Under reduced motion nothing flips: the cell cross-fades to the new text.
 */
export function FlipCell({ from, to, at = 0, reduced = false, track }: { from: string; to: string; at?: number; reduced?: boolean; track?: Array<{ t: number; text: string }> }) {
  // A cell that flips more than once plays a whole track of frames (the motion clip); its text is the track's last.
  if (track && track.length > 1)
    return (
      <span className="flip" data-frames={JSON.stringify(track)} data-at={0} data-reduced={reduced || undefined}>
        {track[track.length - 1]!.text}
      </span>
    );
  if (from === to) return <>{to}</>;
  return (
    <span className="flip" data-frames={JSON.stringify(flipFrames(from, to))} data-at={at} data-reduced={reduced || undefined}>
      {to}
    </span>
  );
}
