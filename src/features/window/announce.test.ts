import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { laneAnnouncement, nextAnnouncement, sessionAnnouncement, statusSentence, switchAnnouncement } from "./announce";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;

describe("sessionAnnouncement: what a screen reader hears about the focused session", () => {
  it("says what a waiting session needs, in the words the prompt line shows", () => {
    expect(sessionAnnouncement(row("needs-permission"))).toBe("billing is waiting for your approval");
    expect(sessionAnnouncement(row("needs-question"))).toBe("infra has a question");
  });
  it("says when a session has ended", () => {
    expect(sessionAnnouncement(row("ended"))).toBe("scratch has ended");
  });
  it("says when a session stopped with an error", () => {
    expect(sessionAnnouncement({ ...row("working"), status: "error" })).toBe("api-server has stopped");
  });
  it("says nothing for a session that is simply working or idle, or for no session", () => {
    expect(sessionAnnouncement(row("working"))).toBe("");
    expect(sessionAnnouncement(row("idle"))).toBe("");
    expect(sessionAnnouncement(null)).toBe("");
  });
});

describe("nextAnnouncement: announce changes, never the first render, never a repeat", () => {
  it("stays silent on the first render, even for a session that is waiting", () => {
    expect(nextAnnouncement(null, row("needs-permission")).text).toBe("");
  });
  it("announces when focus moves to a waiting session: its number and name, then what it needs", () => {
    const first = nextAnnouncement(null, row("working"));
    expect(nextAnnouncement(first, row("needs-permission")).text).toBe("Session 1, billing is waiting for your approval");
  });
  it("announces when the focused session starts waiting", () => {
    const first = nextAnnouncement(null, row("working"));
    const waiting = { ...row("working"), status: "needs-question" as const };
    expect(nextAnnouncement(first, waiting).text).toBe("api-server has a question");
  });
  it("does not repeat: the same session in the same state returns the very same value", () => {
    const first = nextAnnouncement(null, row("working"));
    const a = nextAnnouncement(first, row("needs-permission"));
    expect(nextAnnouncement(a, row("needs-permission"))).toBe(a);
  });
  it("announces the session you switch to, by number and name, even when it has nothing to ask", () => {
    const first = nextAnnouncement(null, row("working"));
    const a = nextAnnouncement(first, row("needs-permission"));
    expect(nextAnnouncement(a, row("idle")).text).toBe("Session 10, docs-site");
  });
  it("says 'No session selected' when the selection is cleared", () => {
    const first = nextAnnouncement(null, row("working"));
    expect(nextAnnouncement(first, null).text).toBe("No session selected");
  });
  it("stays silent about progress: the focused session going from idle to working says nothing", () => {
    const first = nextAnnouncement(null, row("idle"));
    expect(nextAnnouncement(first, { ...row("idle"), status: "working" }).text).toBe("");
  });
  it("on-request mode says nothing by itself, not even on a switch", () => {
    const first = nextAnnouncement(null, row("working"), "on-request");
    expect(nextAnnouncement(first, row("needs-permission"), "on-request").text).toBe("");
  });
});

describe("switchAnnouncement", () => {
  it("names the session by number", () => {
    expect(switchAnnouncement(row("idle"))).toBe("Session 10, docs-site");
    expect(switchAnnouncement(null)).toBe("No session selected");
  });
});

describe("statusSentence: what 'Read the focused session's status' says", () => {
  it("gives the number, the name, the state and the phrase the row shows", () => {
    const r = row("working");
    expect(statusSentence(r)).toBe(`Session ${r.number}, ${r.name}: Working. ${r.phrase}`);
  });
  it("says when nothing is selected", () => {
    expect(statusSentence(null)).toBe("No session selected");
  });
});

describe("laneAnnouncement: a request arriving anywhere is something that needs you", () => {
  it("is silent on the first render and when the count stays or falls", () => {
    expect(laneAnnouncement(null, 2)).toBe("");
    expect(laneAnnouncement(2, 2)).toBe("");
    expect(laneAnnouncement(3, 1)).toBe("");
  });
  it("says how many sessions need you when the count rises", () => {
    expect(laneAnnouncement(1, 2)).toBe("2 sessions need you");
    expect(laneAnnouncement(0, 1)).toBe("1 session needs you");
  });
});
