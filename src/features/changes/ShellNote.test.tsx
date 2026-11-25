import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SHELL_CHANGES_NOTE, ShellNote } from "./ShellNote";

describe("ShellNote", () => {
  it("the note says why Review is off", () => {
    const h = renderToStaticMarkup(<ShellNote text={SHELL_CHANGES_NOTE} />);
    expect(h).toContain("The shell panel runs no agent, so Review is off and comments are copied, not sent.");
    expect(h.match(/<p[ >]/g)).toHaveLength(1);
    expect(h).not.toContain("aria-live");
    expect(h).not.toContain("role=");
  });
});
