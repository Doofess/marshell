import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { RowModel } from "../sidebar/types";
import { ResumeBar } from "./ResumeBar";

const row = (patch: Partial<RowModel> = {}): RowModel => ({ id: "t1", number: 1, name: "Claude Code", project: "billing", projectKey: "billing", agent: "claude", status: "ended", phrase: "Ended last time", ...patch });
const bar = (props: Partial<Parameters<typeof ResumeBar>[0]> = {}) =>
  renderToStaticMarkup(<ResumeBar row={row()} command="claude --continue" folder={"C:\\dev\\billing"} onResume={() => {}} {...props} />);

const text = (html: string) => html.replace(/<[^>]*>/g, "");

describe("ResumeBar", () => {
  it("ended shows the command it will run", () => {
    const h = bar();
    expect(h).toContain("Ended last time. Nothing is running.");
    expect(text(h)).toContain("Resume runs claude --continue in C:\\dev\\billing");
    expect(h).toContain(">Resume</button>");
    // The button is described by the command line.
    const id = /aria-describedby="([^"]+)"/.exec(h)![1]!.split(" ")[0]!;
    expect(h).toMatch(new RegExp(`id="${id}"[^>]*>Resume runs`));
    expect(h).not.toContain("aria-disabled");
  });
  it("exited names the code", () => {
    expect(bar({ row: row({ phrase: "Exited with code 3" }) })).toContain("Exited with code 3.");
  });
  it("a removed profile disables Resume and says why", () => {
    const h = bar({ disabledReason: "Its shell profile was removed." });
    expect(h).toContain('aria-disabled="true"');
    expect(h).toContain("Its shell profile was removed.");
  });
  it("is not a live region", () => {
    expect(bar()).not.toMatch(/role="(alert|status)"|aria-live/);
  });
});
