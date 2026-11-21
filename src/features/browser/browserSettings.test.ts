import { describe, expect, it } from "vitest";
import { BROWSER_DEFAULTS } from "./browserSettings";

describe("browser settings", () => {
  it("the defaults are links on, agent pages on, cookies kept", () => {
    expect(BROWSER_DEFAULTS).toEqual({ terminal_links: true, open_agent_html: true, cookies: "always" });
  });
});
