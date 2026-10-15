import { describe, expect, it } from "vitest";
import { SESSION_NUMBER_HINT, nextSessionNumber, sessionWord } from "./sessionNumber";

describe("nextSessionNumber", () => {
  it("starts at 1", () => expect(nextSessionNumber([])).toBe(1));
  it("takes the lowest free number, so a closed session's number comes back", () => {
    expect(nextSessionNumber([1, 2, 3, 5, 6])).toBe(4);
    expect(nextSessionNumber([2, 3])).toBe(1);
  });
  it("does not renumber: with no gap it takes the next one", () => expect(nextSessionNumber([1, 2, 3])).toBe(4));
  it("ignores values that are not session numbers", () => expect(nextSessionNumber([0, -1, 1.5, Number.NaN, 1])).toBe(2));
  it("reaches past nine; those sessions are found from the palette", () => expect(nextSessionNumber([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])).toBe(12));
});

describe("session words", () => {
  it("names a session the way it is said", () => expect(sessionWord(6)).toBe("Session 6"));
  it("has a hint that teaches the phrase", () => expect(SESSION_NUMBER_HINT).toContain("session 6"));
});
