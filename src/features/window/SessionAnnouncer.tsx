import { useEffect, useRef } from "react";
import { Announcer, useAnnouncer } from "../../components/a11y/Announcer";
import type { AnnounceMode } from "../../components/a11y/announce-policy";
import type { RowModel } from "../sidebar/types";
import { laneAnnouncement, nextAnnouncement, statusSentence, type Announced } from "./announce";

/**
 * The window's single live region (built on the shared Announcer). It is empty on the first render, so nothing is read out
 * as the window opens, and it says something only when focus moves, or the focused session starts waiting, stops or ends.
 * The prompt lines in the panes are plain text; this is the one place anything is announced. `statusRequest` is a counter
 * the "Read the focused session's status" command bumps; each bump reads the focused session's state aloud.
 */
export function SessionAnnouncer({ row, mode = "needs-you", statusRequest = 0, needsYou, notice }: { row: RowModel | null; mode?: AnnounceMode; statusRequest?: number; needsYou?: number; notice?: { id: number; text: string } | null }) {
  const { announcer, message } = useAnnouncer();
  const said = useRef<Announced | null>(null);
  useEffect(() => {
    said.current = nextAnnouncement(said.current, row, mode);
    if (said.current.text) announcer.say(said.current.text);
  }, [row?.id, row?.status, row?.name, mode]); // eslint-disable-line react-hooks/exhaustive-deps -- the row's other fields never change what is said
  // A request arriving in a session that is not focused still needs you: say how many now do.
  const lane = useRef<number | null>(null);
  useEffect(() => {
    if (needsYou === undefined) return;
    const text = laneAnnouncement(lane.current, needsYou);
    lane.current = needsYou;
    if (text && mode !== "on-request") announcer.say(text);
  }, [needsYou]); // eslint-disable-line react-hooks/exhaustive-deps
  const lastRequest = useRef(statusRequest);
  useEffect(() => {
    if (statusRequest === lastRequest.current) return;
    lastRequest.current = statusRequest;
    announcer.say(statusSentence(row), { repeat: true });
  }, [statusRequest]); // eslint-disable-line react-hooks/exhaustive-deps
  // A receipt shown on screen ("Closed Shell 1.") is also said here, so it needs no live region of its own.
  useEffect(() => {
    if (notice) announcer.say(notice.text, { repeat: true });
  }, [notice?.id]); // eslint-disable-line react-hooks/exhaustive-deps -- a new id is a new notice
  return <Announcer message={message} className="window__announcer" data-session-announcer />;
}
