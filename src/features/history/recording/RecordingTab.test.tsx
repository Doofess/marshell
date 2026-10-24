import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { RecordingPart } from "../../../generated/RecordingPart";
import { partFor, RecordingTab, type RecordingState } from "./RecordingTab";

const part = (index: number, over: Partial<RecordingPart> = {}): RecordingPart => ({
  index,
  file: `t1-${1000 + index}.cast`,
  started_ms: 1_000_000 * (index + 1),
  ended_ms: 1_000_000 * (index + 1) + 500_000,
  cols: 80,
  rows: 24,
  span_from_ms: 1_000_000 * (index + 1),
  span_to_ms: 1_000_000 * (index + 1) + 500_000,
  marks_ms: [],
  stopped: null,
  ...over,
});
const frames = async () => ({ cols: 80, rows: 24, screen: "", events: [], next_offset: null });
const html = (parts: RecordingState, request: { turn: number; atMs: number } | null = null) =>
  renderToStaticMarkup(<RecordingTab agent="claude" id="s" parts={parts} turnTimes={[]} request={request} frames={frames} />);
const ready = (...p: RecordingPart[]): RecordingState => ({ status: "ready", parts: p });

describe("RecordingTab", () => {
  it("says it is loading, or that there is no recording, or why it failed", () => {
    expect(html({ status: "loading" })).toContain("Loading the recording…");
    expect(html(ready())).toContain("Marshell has no recording of this conversation.");
    expect(html({ status: "failed", reason: "No such conversation." })).toContain("The recording couldn&#x27;t be read. No such conversation.");
  });
  it("shows the part caption and its buttons only with two or more parts", () => {
    const many = html(ready(part(0), part(1), part(2)));
    expect(many).toMatch(/Part 1 of 3, started /);
    expect(many).toContain("Previous part");
    expect(many).toContain("Next part");
    const one = html(ready(part(0)));
    expect(one).not.toContain("Previous part");
    expect(one).not.toMatch(/Part 1 of/);
  });
  it("opens the part that covers a Play from here request", () => {
    expect(html(ready(part(0), part(1), part(2)), { turn: 1, atMs: 2_100_000 })).toMatch(/Part 2 of 3, started /);
  });
  it("opens the later of two parts that both cover the request, as after a resume in a new tab", () => {
    const older = part(0, { ended_ms: 3_000_000, span_to_ms: 3_000_000 });
    const resumed = part(1);
    expect(html(ready(older, resumed), { turn: 2, atMs: 2_100_000 })).toMatch(/Part 2 of 2, started /);
    expect(partFor([older, resumed], 1_500_000)).toBe(older);
    expect(partFor([older, resumed], 2_100_000)).toBe(resumed);
    expect(partFor([older, resumed], 9_000_000)).toBeUndefined();
  });
  it("leaves a turn that belongs to a later part out of an older part's marks", () => {
    const older = part(0, { ended_ms: 3_000_000, span_to_ms: 3_000_000 });
    const h = renderToStaticMarkup(
      <RecordingTab agent="claude" id="s" parts={ready(older, part(1))} turnTimes={[2_100_000]} request={null} frames={frames} />,
    );
    expect(h).toMatch(/aria-disabled="true">Next mark/);
  });
  it("has the Whole recording checkbox, unchecked", () => {
    const h = html(ready(part(0)));
    expect(h).toContain("Whole recording");
    expect(h).toMatch(/type="checkbox"/);
    expect(h).not.toMatch(/checked/);
  });
  it("shows the stop marker as a note under the player", () => {
    const marker = "Recording stopped: it reached the size limit.";
    expect(html(ready(part(0, { stopped: marker })))).toContain(marker);
    expect(html(ready(part(0)))).not.toContain("Recording stopped");
  });
  it("says a turn outside every part is outside the recording", () => {
    const h = html(ready(part(0), part(1)), { turn: 3, atMs: 99_000_000 });
    expect(h).toContain("This turn is outside the recording.");
    expect(html(ready(part(0)))).not.toContain("outside the recording");
  });
});
