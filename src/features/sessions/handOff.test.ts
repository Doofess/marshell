import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../../generated/SessionInfo";
import { handedOff } from "./handOff";

const infos = (...ids: string[]) => ids.map((tab_id) => ({ tab_id }) as SessionInfo);

describe("handedOff", () => {
  it("is null while the panel's tab isn't listed", () => {
    expect(handedOff("p", undefined, infos("a", "b"))).toBeNull();
  });

  it("is null when there is no panel", () => {
    expect(handedOff(undefined, undefined, infos("a"))).toBeNull();
  });

  it("returns the id once the list holds it", () => {
    expect(handedOff("p", undefined, infos("a", "p"))).toBe("p");
  });

  it("is null when Open in a pane is already completing it", () => {
    expect(handedOff("p", "p", infos("p"))).toBeNull();
  });
});
