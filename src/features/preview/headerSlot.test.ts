import { describe, expect, it } from "vitest";
import { headerPlace, insidePane, slotState } from "./headerSlot";

describe("headerPlace", () => {
  it("draws the pane's own header outside the Files panel", () => {
    expect(headerPlace({ pathShown: false, active: true, slot: "absent" })).toBe("header");
    expect(headerPlace({ pathShown: false, active: false, slot: "ready" })).toBe("header");
  });
  it("in Files only the tab on screen draws its tools, and only into the path bar", () => {
    expect(headerPlace({ pathShown: true, active: true, slot: "ready" })).toBe("slot");
    expect(headerPlace({ pathShown: true, active: false, slot: "ready" })).toBe("none");
    expect(headerPlace({ pathShown: true, active: true, slot: "waiting" })).toBe("none");
  });
  it("a Files tab drawn with no Files panel around it keeps a header, so its tools can be reached", () => {
    expect(headerPlace({ pathShown: true, active: true, slot: "absent" })).toBe("header");
    expect(headerPlace({ pathShown: true, active: false, slot: "absent" })).toBe("header");
  });
  it("tells no panel from a panel whose bar isn't mounted", () => {
    expect(slotState(undefined)).toBe("absent");
    expect(slotState(null)).toBe("waiting");
    expect(slotState({} as HTMLElement)).toBe("ready");
  });
});
describe("insidePane", () => {
  const node = (inside: unknown[]) => ({ contains: (n: unknown) => inside.includes(n) }) as Pick<Node, "contains">;
  const a = {} as Node,
    b = {} as Node,
    c = {} as Node;
  it("counts focus on a tool in the path bar as inside the preview", () => {
    expect(insidePane(node([a]), node([b]), a)).toBe(true);
    expect(insidePane(node([a]), node([b]), b)).toBe(true);
    expect(insidePane(node([a]), node([b]), c)).toBe(false);
    expect(insidePane(node([a]), null, b)).toBe(false);
    expect(insidePane(node([a]), node([b]), null)).toBe(false);
  });
});
