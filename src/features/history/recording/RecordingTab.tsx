import { useState } from "react";
import type { Platform } from "../../../generated/Platform";
import type { RecordingFrames } from "../../../generated/RecordingFrames";
import type { RecordingPart } from "../../../generated/RecordingPart";
import type { TerminalThemeInput } from "../../terminal/terminalTheme";
import { RecordingPlayer, type FramesQuery } from "./RecordingPlayer";

/** What the Recording tab knows about the conversation's recording: asked for, refused, or listed. */
export type RecordingState = { status: "loading" } | { status: "failed"; reason: string } | { status: "ready"; parts: RecordingPart[] };

/** A "Play from here" waiting to be honoured: the turn's number (0-based) and its start on the Unix clock. */
export type PlayRequest = { turn: number; atMs: number };

export const LOADING_COPY = "Loading the recording…";
export const NO_RECORDING_COPY = "Marshell has no recording of this conversation.";
export const OUTSIDE_COPY = "This turn is outside the recording.";

/**
 * The part a moment belongs to: of the parts that cover it, the one that started last. Parts overlap when a conversation is
 * resumed in a new tab while the old tab still records, and from then on the conversation runs in the newer one.
 */
export function partFor<P extends Pick<RecordingPart, "started_ms" | "ended_ms">>(parts: P[], atMs: number): P | undefined {
  let best: P | undefined;
  for (const p of parts) if (atMs >= p.started_ms && atMs <= p.ended_ms && (!best || p.started_ms > best.started_ms)) best = p;
  return best;
}

const whenOf = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

/**
 * The Recording tab: the player for one part at a time, with the part switcher, the "Whole recording" switch (per conversation,
 * not remembered) and the note a stopped recording ends with. `frames` fetches one part's frames; the owner binds the endpoint.
 */
export function RecordingTab({
  agent,
  id,
  parts,
  turnTimes,
  request,
  frames,
  theme,
  font,
  platform,
}: {
  agent: string;
  id: string;
  parts: RecordingState;
  turnTimes: number[];
  request: PlayRequest | null;
  frames: (part: number, q: FramesQuery) => Promise<RecordingFrames>;
  theme?: Omit<TerminalThemeInput, "app">;
  font?: { family: string; size: number };
  platform?: Platform;
}) {
  // A part picked with the buttons holds only for the request it was picked under; a new request picks its own part.
  const [picked, setPicked] = useState<{ request: PlayRequest | null; index: number } | null>(null);
  const [whole, setWhole] = useState(false);

  if (parts.status === "loading") return <p className="recording__note">{LOADING_COPY}</p>;
  if (parts.status === "failed") return <p className="recording__note">The recording couldn&apos;t be read. {parts.reason}</p>;
  const list = parts.parts;
  if (list.length === 0) return <p className="recording__note">{NO_RECORDING_COPY}</p>;

  const covering = request ? partFor(list, request.atMs) : undefined;
  const mine = picked !== null && picked.request === request ? picked.index : null;
  const index = clampIndex(mine ?? covering?.index ?? 0, list.length);
  const part = list[index]!;
  const startAt = mine === null && covering ? request!.atMs : null;
  const go = (by: -1 | 1) => setPicked({ request, index: clampIndex(index + by, list.length) });

  return (
    <section className="recording" aria-label="Recording" data-conversation={`${agent}/${id}`}>
      {request && !covering && <p className="recording__note">{OUTSIDE_COPY}</p>}
      <div className="recording__head">
        {list.length >= 2 && (
          <>
            <button type="button" className="recording__button" aria-disabled={index === 0} onClick={() => go(-1)}>
              Previous part
            </button>
            <button type="button" className="recording__button" aria-disabled={index === list.length - 1} onClick={() => go(1)}>
              Next part
            </button>
            <span className="recording__caption">{`Part ${index + 1} of ${list.length}, started ${whenOf(part.started_ms)}`}</span>
          </>
        )}
        <label className="recording__check">
          <input type="checkbox" checked={whole} onChange={(e) => setWhole(e.target.checked)} />
          <span>Whole recording</span>
        </label>
      </div>
      <RecordingPlayer
        key={`${part.file}|${part.started_ms}`}
        part={part}
        frames={(q) => frames(part.index, q)}
        turnTimes={turnTimes.filter((t) => partFor(list, t) === part)}
        startAtMs={startAt}
        whole={whole}
        theme={theme}
        font={font}
        platform={platform}
      />
      {part.stopped && <p className="recording__stopped">{part.stopped}</p>}
    </section>
  );
}

const clampIndex = (i: number, n: number) => Math.min(Math.max(i, 0), n - 1);
