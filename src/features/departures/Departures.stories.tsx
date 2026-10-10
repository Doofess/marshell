import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReactNode } from "react";
import { ThemePair } from "../../design/ThemePair";
import { FIXTURES } from "../sidebar/fixtures";
import type { RowModel } from "../sidebar/types";
import { SCENARIO_ROWS, SCENARIO_ROWS_WITH_TOOLS } from "../window/scenario";
import { cellAt, CLIP_DURATION_MS, CLIP_EVENTS, clipTrack } from "./clip";
import { boardRows } from "./board";
import { DeparturesBoard, type DeparturesBoardProps } from "./DeparturesBoard";

const meta: Meta = { title: "Departures", parameters: { layout: "fullscreen" } };
export default meta;

const byId = (...ids: string[]) => ids.map((id) => FIXTURES.find((f) => f.id === id)!);
const frame = (children: ReactNode, width = 1180) => <div style={{ inlineSize: width, maxInlineSize: "100%" }}>{children}</div>;
const board = (rows: RowModel[], heightPx: number, extra: Partial<DeparturesBoardProps> = {}, width?: number) => frame(<DeparturesBoard rows={rows} heightPx={heightPx} {...extra} />, width);

/** Twelve sessions: two gate calls, the rest by number, and the ones that do not fit counted, never half shown. */
const TWELVE: RowModel[] = byId("needs-permission", "needs-question", "working", "done-unseen", "error", "idle", "done-seen", "stuck", "ended", "limited", "muted", "elevated");

export const TwelveSessions: StoryObj = { render: () => <ThemePair label="Twelve sessions at 520 px: the two that need you on top, then the others by number, and the rest counted with an All sessions button" stack>{board(TWELVE, 520, { active: 0 })}</ThemePair> };
export const FiveAgentScenario: StoryObj = { render: () => <ThemePair label="The five-agent scenario: billing has waited longest, so it is first among the gate calls" stack>{board(SCENARIO_ROWS, 560, { active: 0 })}</ThemePair> };
export const WithManualCli: StoryObj = { render: () => <ThemePair label="Plain terminals on the board: AT PROMPT, RUNNING and EXITED, an en dash for context, no thinking motion" stack>{board(SCENARIO_ROWS_WITH_TOOLS, 700, { active: 5 })}</ThemePair> };
export const NoSessions: StoryObj = { render: () => <ThemePair label="No sessions: one sentence and one way forward" stack>{board([], 320)}</ThemePair> };
export const InlineCard: StoryObj = { render: () => <ThemePair label="Y or N on a gate call opens its request inline, under the row; Enter only jumps to the session" stack>{board(SCENARIO_ROWS, 760, { active: 0, card: "needs-permission" })}</ThemePair> };
export const Narrow: StoryObj = { render: () => <ThemePair label="At 720 px wide the context and recap columns go, and the five that name and date a session stay whole" stack>{board(SCENARIO_ROWS_WITH_TOOLS, 700, { active: 1 }, 720)}</ThemePair> };
export const RtlNames: StoryObj = { render: () => <ThemePair label="Right-to-left names, a CJK name and an emoji name read in their own direction in the same column" stack>{board(byId("rtl", "cjk-emoji", "emoji-name", "working"), 360)}</ThemePair> };
export const LongNames: StoryObj = { render: () => <ThemePair label="Very long names, projects and branches end in an ellipsis; the number, status and waiting time never do" stack>{board(byId("long-names", "crowded", "tiny", "working"), 360)}</ThemePair> };
export const ReducedMotion: StoryObj = { render: () => <ThemePair label="Reduced motion: a changed status or waiting time cross-fades in 200 ms, with no flipping" stack>{board(SCENARIO_ROWS, 560, { reduced: true, flips: { working: { status: { from: "WORKING", at: 0 }, waiting: { from: "1 MIN", at: 0 } } } })}</ThemePair> };

const CLIP_ROWS = SCENARIO_ROWS.map((r) => (r.id === "working" ? { ...r } : r));
const baseline = (rowId: string, cell: "status" | "waiting") => {
  const r = boardRows(CLIP_ROWS).find((x) => x.id === rowId)!;
  return cell === "status" ? r.statusText : r.waitingText;
};
const clipTracks: NonNullable<DeparturesBoardProps["tracks"]> = {};
for (const e of CLIP_EVENTS) {
  clipTracks[e.rowId] ??= {};
  clipTracks[e.rowId]![e.cell] = clipTrack(e.rowId, e.cell, baseline(e.rowId, e.cell));
}

export const MotionClip: StoryObj = {
  render: () => (
    <ThemePair label="10 seconds: at 1 s a session starts waiting, at 3.2 s a wait grows, at 5 s an error resumes, at 8 s an answer arrives. Press Replay animations." stack>
      {board(CLIP_ROWS, 560, { tracks: clipTracks })}
    </ThemePair>
  ),
};

const FILM_TIMES = [0, 1_100, 3_300, 5_100, 6_600, 8_100];
export const MotionFilmstrip: StoryObj = {
  render: () => (
    <ThemePair label="The same clip as six still frames, so it can be judged without playing it" stack>
      <div style={{ display: "grid", gap: 16 }}>
        {FILM_TIMES.map((t) => {
          const texts: NonNullable<DeparturesBoardProps["texts"]> = {};
          for (const e of CLIP_EVENTS) {
            texts[e.rowId] ??= {};
            texts[e.rowId]![e.cell] = cellAt(e.rowId, e.cell, baseline(e.rowId, e.cell), t);
          }
          return (
            <figure key={t} style={{ margin: 0 }}>
              <figcaption style={{ color: "var(--text-2)", fontSize: 12, marginBlockEnd: 4 }}>
                {(t / 1000).toFixed(1)} s of {CLIP_DURATION_MS / 1000}
              </figcaption>
              {board(CLIP_ROWS, 330, { texts }, 900)}
            </figure>
          );
        })}
      </div>
    </ThemePair>
  ),
};
