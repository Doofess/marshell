import { describe, expect, it } from "vitest";
import type { Scheme } from "./palettes";
import { trackColorSchemeMode, type SchemeTerm } from "./colorSchemeMode";

type Params = (number | number[])[];
type Csi = { id: { prefix?: string; final: string }; fn: (p: Params) => boolean };
type Esc = { id: { final: string }; fn: () => boolean };

function setup(start: Scheme = "dark") {
  const csi: Csi[] = [];
  const esc: Esc[] = [];
  const term: SchemeTerm = {
    parser: {
      registerCsiHandler: (id, fn) => (csi.push({ id, fn: (p) => fn(p) as boolean }), { dispose: () => {} }),
      registerEscHandler: (id, fn) => (esc.push({ id, fn: () => fn() as boolean }), { dispose: () => {} }),
    },
  };
  const sent: string[] = [];
  let scheme = start;
  const mode = trackColorSchemeMode(term, (d) => sent.push(d), () => scheme);
  const send = (final: string, ...params: number[]) => csi.find((h) => h.id.prefix === "?" && h.id.final === final)!.fn(params);
  const reset = () => esc.find((h) => h.id.final === "c")!.fn();
  const flip = (s: Scheme) => {
    scheme = s;
    mode.notify(s);
  };
  return { sent, send, reset, flip };
}

describe("mode 2031", () => {
  it("reports nothing before 2031 is set", () => {
    const t = setup("dark");
    t.flip("light");
    expect(t.sent).toEqual([]);
  });
  it("sends 997;2 for light after 2031 h", () => {
    const t = setup("dark");
    t.send("h", 2031);
    t.flip("light");
    expect(t.sent).toEqual(["\x1b[?997;2n"]);
  });
  it("sends 997;1 for dark after 2031 h", () => {
    const t = setup("light");
    t.send("h", 2031);
    t.flip("dark");
    expect(t.sent).toEqual(["\x1b[?997;1n"]);
  });
  it("sends no second report for the same scheme", () => {
    const t = setup("dark");
    t.send("h", 2031);
    t.flip("dark");
    t.flip("light");
    t.flip("light");
    expect(t.sent).toEqual(["\x1b[?997;2n"]);
  });
  it("finds 2031 among other modes", () => {
    const t = setup("dark");
    t.send("h", 1049, 2031);
    t.flip("light");
    expect(t.sent).toHaveLength(1);
  });
  it("answers the 996 query at once even without 2031", () => {
    const t = setup("light");
    expect(t.send("n", 996)).toBe(true);
    expect(t.sent).toEqual(["\x1b[?997;2n"]);
  });
  it("leaves other DSR queries to xterm", () => {
    const t = setup();
    expect(t.send("n", 6)).toBe(false);
    expect(t.sent).toEqual([]);
  });
  it("stops reports after 2031 l", () => {
    const t = setup("dark");
    t.send("h", 2031);
    t.send("l", 2031);
    t.flip("light");
    expect(t.sent).toEqual([]);
  });
  it("stops reports after a full reset", () => {
    const t = setup("dark");
    t.send("h", 2031);
    expect(t.reset()).toBe(false);
    t.flip("light");
    expect(t.sent).toEqual([]);
  });
  it("lets xterm continue for h and l", () => {
    const t = setup();
    expect(t.send("h", 2031)).toBe(false);
    expect(t.send("l", 2031)).toBe(false);
  });
});
