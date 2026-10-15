import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { paletteSessions } from "./sessions";

describe("paletteSessions: the sidebar's rows, as the palette lists them", () => {
  const out = paletteSessions(FIXTURES);
  it("keeps one entry per row, with the row's own number, name, folder, mark and state", () => {
    expect(out).toHaveLength(FIXTURES.length);
    FIXTURES.forEach((r, i) =>
      expect(out[i]).toEqual({ id: r.id, number: r.number, name: r.name, project: r.project, branch: r.branch, kind: r.agent, status: r.status }),
    );
  });
  it("never renumbers: the number a row holds is the number the palette shows", () => {
    expect(out.map((s) => s.number)).toEqual(FIXTURES.map((r) => r.number));
  });
});
