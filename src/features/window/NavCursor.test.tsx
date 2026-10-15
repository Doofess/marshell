import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { NO_SELECTION_TEXT, NavAnnouncer, NavCursor, navHint } from "./NavCursor";

describe("NavCursor", () => {
  const rect = { x: 288, y: 40, w: 300, h: 300 };
  it("draws nothing when there is no cursor", () => {
    expect(renderToStaticMarkup(<NavCursor rect={null} label="" os="windows" />)).toBe("");
  });
  it("is a picture over the target: hidden from screen readers, placed by its measured rectangle", () => {
    const h = renderToStaticMarkup(<NavCursor rect={rect} label="Session 3, api-server" os="windows" />);
    expect(h).toContain('class="nav-cursor"');
    expect(h).toContain('aria-hidden="true"');
    expect(h).toMatch(/inset-inline-start:288px/);
    expect(h).toMatch(/inline-size:300px/);
    expect(h).toContain("Session 3, api-server");
  });
  it("says what releasing does, with the modifier named for the system", () => {
    expect(navHint("windows")).toBe("Release Ctrl+Shift to select · Backspace for none");
    expect(navHint("mac")).toBe("Release ⌃⇧ to select · Backspace for none");
    expect(navHint("linux", "ctrl+alt")).toBe("Release Ctrl+Alt to select · Backspace for none");
    expect(renderToStaticMarkup(<NavCursor rect={rect} label="x" os="mac" />)).toContain("Release ⌃⇧ to select");
  });
});

describe("NavAnnouncer", () => {
  it("is one polite live region that says what was selected", () => {
    const h = renderToStaticMarkup(<NavAnnouncer text="Selected session 6, billing" />);
    expect(h).toContain('role="status"');
    expect(h).toContain('aria-live="polite"');
    expect(h).toContain("Selected session 6, billing");
  });
  it("is empty until something is said, so nothing is read out as the window opens", () => {
    expect(renderToStaticMarkup(<NavAnnouncer text="" />)).toContain("></div>");
  });
});

describe("the empty selection", () => {
  it("tells you how to select a terminal", () => {
    expect(NO_SELECTION_TEXT).toBe("No session selected · Tab to select a terminal");
  });
});
