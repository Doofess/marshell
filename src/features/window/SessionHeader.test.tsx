import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { SessionHeader, headerDanger, modeLabel } from "./SessionHeader";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;

describe("modeLabel / headerDanger", () => {
  it("names each mode", () => {
    expect(modeLabel("manual")).toBe("Manual");
    expect(modeLabel("plan")).toBe("Plan");
    expect(modeLabel("auto-edit")).toBe("Auto-edit");
    expect(modeLabel("full-auto")).toBe("Full auto");
    expect(modeLabel("bypass")).toBe("Bypass");
    expect(modeLabel(undefined)).toBe("–");
  });
  it("outlines the header in red only for bypass and auto", () => {
    expect(headerDanger("bypass")).toBe(true);
    expect(headerDanger("full-auto")).toBe(true);
    expect(headerDanger("auto-edit")).toBe(false);
    expect(headerDanger("plan")).toBe(false);
    expect(headerDanger(undefined)).toBe(false);
  });
});

describe("SessionHeader", () => {
  const h = renderToStaticMarkup(<SessionHeader row={row("working")} ports={["localhost:3000"]} />);
  it("shows name, project and branch, mode, model and effort, subagents, context and ports in one bar", () => {
    for (const s of ["api-server", "my-app", "feat/auth-flow", "Plan", "Opus 5.5", "high", "2 subagents", "Context 42% used", "localhost:3000"]) expect(h, s).toContain(s);
  });
  it("leaves out what is unknown: no dash, no guess (the user's rule for every unknown value)", () => {
    const u = renderToStaticMarkup(<SessionHeader row={row("unknown")} />);
    expect(u).not.toContain("–");
    expect(u).not.toContain("subagents");
    expect(u).not.toContain("session-header__meta");
    expect(u).not.toContain('class="chip"');
  });
  it("shows a model alone, or an effort alone, without a dangling separator", () => {
    const m = renderToStaticMarkup(<SessionHeader row={{ ...row("unknown"), model: "Opus 5.5" }} />);
    expect(m).toContain("Opus 5.5");
    expect(m).not.toContain("Opus 5.5 ·");
    const e = renderToStaticMarkup(<SessionHeader row={{ ...row("unknown"), effort: "high" }} />);
    expect(e).toContain(">high<");
    expect(e).not.toContain("· high");
  });
  it("says the mode is the one the session started in, since a change inside the CLI is not tracked yet", () => {
    const label = 'aria-label="Started in Plan. A change made inside the CLI isn&#x27;t shown yet."';
    expect(h).toContain(label);
    expect(h).toContain('title="Started in Plan. A change made inside the CLI isn&#x27;t shown yet."');
  });
  it("is marked red for a bypass session", () => {
    expect(renderToStaticMarkup(<SessionHeader row={row("bypass")} />)).toContain("data-danger");
    expect(h).not.toContain("data-danger");
  });
  it("labels an elevated session for the whole header", () => {
    expect(renderToStaticMarkup(<SessionHeader row={row("elevated")} />)).toContain("Administrator");
  });
  it("is a labelled banner-level region", () => {
    expect(h).toContain('role="group"');
    expect(h).toContain('aria-label="Session 3, api-server"');
  });
});
