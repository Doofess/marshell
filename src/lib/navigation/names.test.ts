import { expect, it } from "vitest";
import { topName } from "./names";

it("names every title-bar target", () => {
  expect(["palette", "sidebar", "launcher", "panel-browser", "panel-files"].map(topName)).toEqual(["Command palette", "Sidebar", "New session", "Browser", "Files"]);
  expect(topName("brand")).toBe("brand");
});
