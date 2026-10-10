import { describe, expect, it } from "vitest";
import { COMMANDS } from "../../lib/keymap";
import { filterCommands, filterProjects, filterSessions, highlight, matchRanges, parseSessionNumber, pickByLetter, projectLabel, sessionDetail, type Project, type SessionRef } from "./filter";

const P = (id: string, name: string, branch: string | undefined, lastUsedMs: number): Project => ({ id, name, branch, path: `C:/dev/${name}`, lastUsedMs });
const projects = [P("a", "my-app", "main", 300), P("b", "app-server", "develop", 200), P("c", "snapshot", undefined, 100), P("d", "marketing", "feat/pricing", 400)];

describe("matchRanges", () => {
  it("matches a subsequence, case-insensitively, and reports where", () => {
    expect(matchRanges("MA", "my-app")).toEqual([[0, 1], [3, 4]]);
    expect(matchRanges("app", "my-app")).toEqual([[3, 6]]);
  });
  it("returns null when the text does not contain the query's letters in order", () => {
    expect(matchRanges("zz", "my-app")).toBeNull();
    expect(matchRanges("ppa", "my-app")).toBeNull();
  });
  it("matches everything, with nothing to highlight, for an empty or blank query", () => {
    expect(matchRanges("", "x")).toEqual([]);
    expect(matchRanges("   ", "x")).toEqual([]);
  });
  it("counts code points, so emoji and CJK ranges line up (review focus 5)", () => {
    expect(matchRanges("\u{1F680}", "go \u{1F680} now")).toEqual([[3, 4]]);
    expect(matchRanges("\u8a8d", "\u7d71\u8a8d\u8a3c")).toEqual([[1, 2]]);
  });
});

describe("filterProjects (review focus 5)", () => {
  it("returns everything in the remembered order for an empty query", () => {
    expect(filterProjects("", projects).map((h) => h.item.id)).toEqual(["d", "a", "b", "c"]);
    expect(filterProjects("  ", projects).map((h) => h.item.id)).toEqual(["d", "a", "b", "c"]);
  });
  it("never throws on characters that look like a pattern", () => {
    for (const q of ["(", "[", "\\", ".*", "+", "?", "$", "^", "{1,2}", "|", "\u{1F680}"]) expect(() => filterProjects(q, projects), q).not.toThrow();
  });
  it("treats those characters literally", () => {
    expect(filterProjects(".*", projects)).toEqual([]);
    expect(filterProjects("feat/pr", projects).map((h) => h.item.id)).toEqual(["d"]);
  });
  it("ranks a word-start match above a scattered one", () => {
    expect(filterProjects("ap", projects).map((h) => h.item.id)[0]).toBe("b");
  });
  it("breaks equal scores by most recently used", () => {
    const same = [P("x", "web", undefined, 1), P("y", "web", undefined, 9)];
    expect(filterProjects("web", same).map((h) => h.item.id)).toEqual(["y", "x"]);
  });
  it("drops projects that do not match", () => {
    expect(filterProjects("zzz", projects)).toEqual([]);
  });
  it("labels a project with its branch", () => {
    expect(projectLabel(projects[0]!)).toBe("my-app \u00b7 main");
    expect(projectLabel(projects[2]!)).toBe("snapshot");
  });
  it("returns ranges against the label", () => {
    const hit = filterProjects("main", projects)[0]!;
    expect(highlight(projectLabel(hit.item), hit.ranges).filter((s) => s.hit).map((s) => s.text).join("")).toBe("main");
  });
});

describe("filterCommands", () => {
  it("lists every command, in order, for an empty query", () => {
    expect(filterCommands("", COMMANDS).map((h) => h.item.id)).toEqual(COMMANDS.map((c) => c.id));
  });
  it("finds Split view by typing split", () => {
    expect(filterCommands("split", COMMANDS)[0]!.item.id).toBe("split");
  });
  it("finds a command by scattered letters", () => {
    expect(filterCommands("stse", COMMANDS).map((h) => h.item.id)).toContain("launcher");
  });
  it("returns nothing for a query that matches nothing", () => {
    expect(filterCommands("qqqq", COMMANDS)).toEqual([]);
  });
});

describe("highlight", () => {
  it("splits text into hit and plain segments", () => {
    expect(highlight("my-app", [[3, 6]])).toEqual([{ text: "my-", hit: false }, { text: "app", hit: true }]);
  });
  it("returns the whole text as one plain segment with no ranges", () => {
    expect(highlight("abc", [])).toEqual([{ text: "abc", hit: false }]);
  });
  it("keeps emoji whole", () => {
    expect(highlight("a\u{1F680}b", [[1, 2]])).toEqual([{ text: "a", hit: false }, { text: "\u{1F680}", hit: true }, { text: "b", hit: false }]);
  });
});

describe("pickByLetter", () => {
  const labels = ["Claude", "Codex", "Gemini"];
  it("picks the first option starting with the letter", () => {
    expect(pickByLetter(labels, "g")).toBe(2);
    expect(pickByLetter(labels, "G")).toBe(2);
  });
  it("cycles through options that share the letter", () => {
    expect(pickByLetter(labels, "c", -1)).toBe(0);
    expect(pickByLetter(labels, "c", 0)).toBe(1);
    expect(pickByLetter(labels, "c", 1)).toBe(0);
  });
  it("returns -1 when nothing matches or the key is not a single letter", () => {
    expect(pickByLetter(labels, "x")).toBe(-1);
    expect(pickByLetter(labels, "1")).toBe(-1);
    expect(pickByLetter(labels, "Enter")).toBe(-1);
    expect(pickByLetter(labels, "")).toBe(-1);
  });
});

describe("parseSessionNumber: saying session 6 is a way of saying 6", () => {
  it("reads a bare number, #6, and the phrases people say", () => {
    for (const q of ["6", "#6", " 6 ", "session 6", "Session #6", "go to session 6", "Go To Session6"]) expect(parseSessionNumber(q), q).toBe(6);
  });
  it("reads two and three digit numbers", () => {
    expect(parseSessionNumber("12")).toBe(12);
    expect(parseSessionNumber("#103")).toBe(103);
  });
  it("is null for anything else, zero included", () => {
    for (const q of ["", "0", "#", "6a", "a6", "session", "split 6", "1 2", "-3", "1.5", "\u0663"]) expect(parseSessionNumber(q), q).toBeNull();
  });
});

const S = (number: number, name: string, project: string, branch?: string): SessionRef => ({ id: `s${number}`, number, name, project, branch });
const sessions = [
  S(1, "api-server", "my-app", "feat/auth-flow"),
  S(2, "billing", "payments", "fix/retry-backoff"),
  S(3, "infra", "terraform"),
  S(5, "docs", "my-app", "main"),
  S(6, "ssh prod-6", "ops"),
  S(7, "scratch", "tools", "main"),
];

describe("filterSessions", () => {
  it("lists every session by number for an empty query", () => {
    expect(filterSessions("", sessions).map((h) => h.item.number)).toEqual([1, 2, 3, 5, 6, 7]);
  });
  it("puts the session with that number first when the query is a number, then name matches", () => {
    const hits = filterSessions("6", sessions);
    expect(hits[0]!.item.number).toBe(6);
    expect(hits[0]!.exact).toBe(true);
    expect(hits.slice(1).every((h) => !h.exact)).toBe(true);
  });
  it("understands #6 and session 6 the same way", () => {
    for (const q of ["#6", "session 6", "go to session 6"]) expect(filterSessions(q, sessions)[0]!.item.id, q).toBe("s6");
  });
  it("finds nothing exact for a number nobody has", () => {
    expect(filterSessions("#4", sessions).some((h) => h.exact)).toBe(false);
  });
  it("matches by name and by project and branch, highlighting the part that matched", () => {
    const byName = filterSessions("bill", sessions);
    expect(byName.map((h) => h.item.id)).toEqual(["s2"]);
    expect(highlight("billing", byName[0]!.ranges).filter((s) => s.hit).map((s) => s.text).join("")).toBe("bill");
    const byDetail = filterSessions("terraf", sessions);
    expect(byDetail[0]!.item.id).toBe("s3");
    expect(byDetail[0]!.ranges).toEqual([]);
    expect(highlight(sessionDetail(byDetail[0]!.item), byDetail[0]!.detailRanges).some((s) => s.hit)).toBe(true);
  });
  it("breaks equal matches by lower number", () => {
    expect(filterSessions("main", sessions).map((h) => h.item.number)).toEqual([5, 7]);
  });
  it("never throws on pattern-like input", () => {
    for (const q of ["(", "[", "\\", ".*", "#(", "\u{1F680}"]) expect(() => filterSessions(q, sessions), q).not.toThrow();
  });
  it("describes a session's project and branch", () => {
    expect(sessionDetail(sessions[0]!)).toBe("my-app \u00b7 feat/auth-flow");
    expect(sessionDetail(sessions[2]!)).toBe("terraform");
  });
});
