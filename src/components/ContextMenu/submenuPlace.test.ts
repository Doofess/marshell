import { describe, expect, it } from "vitest";
import { submenuPlace } from "./submenuPlace";

describe("submenuPlace", () => {
  const vp = { w: 1200, h: 800 };
  const size = { w: 180, h: 200 };
  it("the_submenu_opens_beside_the_menu_or_on_the_other_side", () => {
    expect(submenuPlace({ left: 14, right: 214 }, 600, size, vp)).toEqual({ x: 216, y: 592 });
    expect(submenuPlace({ left: 1000, right: 1190 }, 100, size, vp)).toEqual({ x: 818, y: 100 });
  });
});
