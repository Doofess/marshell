import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { RecordingPart } from "../../../generated/RecordingPart";
import { RecordingPlayer } from "./RecordingPlayer";

const part: RecordingPart = {
  index: 0,
  file: "t1-1000.cast",
  started_ms: 1_000_000,
  ended_ms: 1_083_000,
  cols: 80,
  rows: 24,
  span_from_ms: 1_000_000,
  span_to_ms: 1_083_000,
  marks_ms: [],
  stopped: null,
};
const frames = async () => ({ cols: 80, rows: 24, screen: "", events: [], next_offset: null });
const html = (over: { whole?: boolean; startAtMs?: number | null; part?: RecordingPart } = {}) =>
  renderToStaticMarkup(<RecordingPlayer part={over.part ?? part} frames={frames} turnTimes={[]} startAtMs={over.startAtMs ?? null} whole={over.whole ?? false} />);

describe("RecordingPlayer controls", () => {
  const h = html();
  it("is a group labelled Recording player with an empty terminal host", () => {
    expect(h).toContain('aria-label="Recording player"');
    expect(h).toContain('aria-label="Recording screen"');
  });
  it("has Play, Previous mark and Next mark", () => {
    expect(h).toContain(">Play<");
    expect(h).toContain(">Previous mark<");
    expect(h).toContain(">Next mark<");
  });
  it("has a Position slider with the time as text", () => {
    expect(h).toContain('aria-label="Position"');
    expect(h).toContain('aria-valuetext="0:00 of 1:23"');
  });
  it("starts at the requested time, within the range", () => {
    expect(html({ startAtMs: 1_030_000 })).toContain('aria-valuetext="0:30 of 1:23"');
    expect(html({ startAtMs: 5 })).toContain('aria-valuetext="0:00 of 1:23"');
  });
  it("plays the whole part when asked, not only the conversation's span", () => {
    const p = { ...part, span_from_ms: 1_010_000, span_to_ms: 1_020_000 };
    expect(html({ part: p })).toContain('aria-valuetext="0:00 of 0:10"');
    expect(html({ part: p, whole: true })).toContain('aria-valuetext="0:00 of 1:23"');
  });
  it("lists the speeds and idle caps in order, with 1× and Up to 2 s selected", () => {
    const options = (aria: string) => {
      const m = new RegExp(`aria-label="${aria}"[^>]*>(.*?)</select>`).exec(h);
      return [...(m?.[1] ?? "").matchAll(/<option[^>]*>([^<]*)</g)].map((x) => x[1]);
    };
    expect(options("Speed")).toEqual(["0.5×", "1×", "2×", "4×"]);
    expect(options("Idle time")).toEqual(["Keep", "Up to 1 s", "Up to 2 s", "Up to 5 s"]);
    expect(h).toMatch(/<option value="1" selected="">1×/);
    expect(h).toMatch(/selected="">Up to 2 s/);
  });
});
