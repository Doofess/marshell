import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { APPROVALS } from "./fixtures";
import { ApproveCard } from "./ApproveCard";
import type { ApprovalRequest } from "./types";

const html = (r: ApprovalRequest, p: Partial<Parameters<typeof ApproveCard>[0]> = {}) => renderToStaticMarkup(<ApproveCard request={r} {...p} />);

describe("ApproveCard: a safe request", () => {
  const h = html(APPROVALS.safeBash);
  it("says who, what and for how long", () => {
    expect(h).toContain("billing");
    expect(h).toContain("Run a command");
    expect(h).toContain("1m");
    expect(h).toContain("npm test");
  });
  it("offers Deny, Allow once, the exact Always rule and Answer in terminal, each with its key", () => {
    expect(h).toContain("Deny");
    expect(h).toContain("Allow once");
    expect(h).toContain("Bash(npm test:*)");
    expect(h).toContain("in this project");
    expect(h).toContain("Answer in terminal");
    for (const k of ["N", "Y", "A", "T"]) expect(h).toContain(`<kbd class="approve-card__kbd" aria-hidden="true">${k}</kbd>`);
  });
  it("is a labelled group a screen reader can find", () => {
    expect(h).toContain('role="group"');
    expect(h).toMatch(/aria-label="billing: Run a command/);
    expect(h).toContain('tabindex="0"');
  });
  it("does not show the cwd when it matches the session's", () => {
    expect(h).not.toContain("approve-card__cwd");
  });
});

describe("ApproveCard: a risky request", () => {
  const h = html(APPROVALS.riskyBash);
  it("states the plain reason, with a warning glyph", () => {
    expect(h).toContain("Deletes files recursively outside this project");
    expect(h).toContain('data-kind="caution"');
    expect(h).toContain('data-risk="risky"');
  });
  it("turns Allow into a hold with a progress ring, and hides Always", () => {
    expect(h).toContain("Hold to allow");
    expect(h).toContain("approve-card__ring");
    expect(h).not.toContain("Always allow");
  });
  it("shows the cwd when it differs from the session's", () => {
    expect(h).toContain("approve-card__cwd");
    expect(h).toContain("C:/dev/my-app/packages/web");
  });
  it("draws the hold progress", () => {
    expect(html(APPROVALS.riskyBash, { holdProgress: 0.5 })).toContain('stroke-dasharray="18.84955592153876 37.69911184307752"');
  });
});

describe("ApproveCard: payloads", () => {
  it("collapses a 40-line command and offers the real total", () => {
    const h = html(APPROVALS.longCommand);
    expect(h).toContain("data-collapsed");
    expect(h).toContain("Show all 40 lines (Space)");
  });
  it("shows everything once expanded", () => {
    const h = html(APPROVALS.longCommand, { expanded: true });
    expect(h).not.toContain("data-collapsed");
    expect(h).not.toContain("Show all");
  });
  it("marks diff lines with a sign as well as a colour", () => {
    const h = html(APPROVALS.edit);
    expect(h).toContain('data-kind="add"');
    expect(h).toContain('data-kind="del"');
    expect(h).toContain("+ ");
    expect(h).toContain("- ");
    expect(h).toContain("Edit src/auth.ts (+12 \u22123)");
  });
  it("shows MCP arguments as key: value", () => {
    expect(html(APPROVALS.mcp)).toContain("title: Upgrade aws provider to 6.x");
  });
  it("labels a new file as Create", () => {
    expect(html(APPROVALS.writeNew)).toContain("Create .env.example");
  });
  it("renders hostile text as text, never as markup (review focus 1)", () => {
    const evil: ApprovalRequest = { ...APPROVALS.safeBash, detail: { kind: "bash", command: `echo "<img src=x onerror=alert(1)>" ${"A".repeat(300)} \u8a8d\u8a3c \u062e\u0627\u062f\u0645` } };
    const h = html(evil);
    expect(h).not.toContain("<img");
    expect(h).toContain("&lt;img");
    expect(h).toContain('dir="auto"');
  });
});

describe("ApproveCard: a question", () => {
  const h = html(APPROVALS.question);
  it("shows the question and one button per answer, with no Allow", () => {
    expect(h).toContain("Which AWS region should the new provider config target?");
    for (const o of ["us-east-1", "eu-west-1", "ap-southeast-2"]) expect(h).toContain(o);
    expect(h).not.toContain("Allow once");
  });
  it("still offers the terminal", () => {
    expect(h).toContain("Answer in terminal");
  });
});

describe("ApproveCard: after the request", () => {
  it("shows a one-line receipt, no buttons and no undo", () => {
    const h = html(APPROVALS.receipt);
    expect(h).toContain('role="status"');
    expect(h).toContain("Allowed \u00b7 npm test");
    expect(h).not.toContain("<button");
    expect(h.toLowerCase()).not.toContain("undo");
  });
  it("shows a denied receipt", () => {
    const denied: ApprovalRequest = { ...APPROVALS.receipt, state: { phase: "decided", verdict: "denied" } };
    expect(html(denied)).toContain("Denied \u00b7 npm test");
  });
  it("says it was answered in the terminal", () => {
    expect(html(APPROVALS.answeredInTerminal)).toContain("Answered in terminal \u00b7 npm test");
  });
  it("says a timed-out request moved to the terminal, naming the agent and the wait", () => {
    const h = html(APPROVALS.released);
    expect(h).toContain("Timed out after 2m");
    expect(h).toContain("Claude is asking in its own terminal now");
  });
  it("can mark the receipt as leaving", () => {
    expect(html(APPROVALS.receipt, { exiting: true })).toContain("data-exiting");
  });
});

describe("ApproveCard: arming", () => {
  it("marks every action as inert before the delay ends", () => {
    const h = html(APPROVALS.safeBash, { armed: false });
    expect((h.match(/aria-disabled="true"/g) ?? []).length).toBeGreaterThanOrEqual(4);
  });
  it("is live once armed", () => {
    expect(html(APPROVALS.safeBash)).not.toContain('aria-disabled="true"');
  });
});

describe("ApproveCard: markup hygiene", () => {
  it("never puts a literal colour in markup", () => {
    for (const r of Object.values(APPROVALS)) expect(html(r)).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("never uses clever copy", () => {
    for (const r of Object.values(APPROVALS)) {
      const h = html(r).toLowerCase();
      expect(h).not.toContain("successfully");
      expect(h).not.toContain("oops");
    }
  });
});

describe("ApproveCard: the vendor's colour", () => {
  it("carries the session's agent, so its badge and receipt glyphs take the vendor's colour like the rows do", () => {
    expect(html(APPROVALS.safeBash)).toContain('data-agent="claude"');
    expect(html(APPROVALS.longCommand)).toContain('data-agent="gemini"');
    expect(html(APPROVALS.writeNew)).toContain('data-agent="codex"');
    for (const r of [APPROVALS.receipt, APPROVALS.answeredInTerminal, APPROVALS.released]) expect(html(r)).toContain(`data-agent="${r.session.agent}"`);
  });
});
