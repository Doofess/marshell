import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { scriptsFixture } from "./fixtures";
import { ScriptsTrust } from "./ScriptsTrust";

const h = (over = {}, extra = {}) => renderToStaticMarkup(<ScriptsTrust scripts={scriptsFixture(over)} allowLabel="Allow and run" cancelLabel="Cancel" onAllow={() => {}} onCancel={() => {}} {...extra} />);

describe("ScriptsTrust", () => {
  it("has the heading, the line naming the file's folder, and the two buttons", () => {
    const out = h();
    expect(out).toContain("Allow this repo&#x27;s scripts?");
    expect(out).toContain("marshell.json in C:/dev/app runs these commands on this computer:");
    expect(out).toContain(">Allow and run<");
    expect(out).toContain(">Cancel<");
    expect(out.indexOf("Allow and run")).toBeLessThan(out.indexOf(">Cancel<"));
  });
  it("lists only the present scripts, each as text in a code element", () => {
    const out = h({ run: null });
    expect(out).toContain("<code dir=\"ltr\">setup: pnpm install</code>");
    expect(out).not.toContain("run: ");
    expect(out).not.toContain("teardown: ");
    expect(h({ teardown: "rm -rf dist" })).toContain("<code dir=\"ltr\">teardown: rm -rf dist</code>");
  });
  it("shows a script as text, never as markup", () => {
    const out = h({ setup: "<img src=x onerror=alert(1)>" });
    expect(out).not.toContain("<img");
    expect(out).toContain("&lt;img");
  });
  it("makes both buttons unavailable while busy, without the disabled attribute", () => {
    const out = h({}, { busy: true });
    expect(out.match(/aria-disabled="true"/g)?.length).toBe(2);
    expect(out).not.toMatch(/\sdisabled=/);
  });
});
