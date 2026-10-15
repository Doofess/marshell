import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ThemePair } from "../../design/ThemePair";
import { APPROVALS } from "./fixtures";
import { ApproveCard } from "./ApproveCard";
import { useArming } from "./useArming";
import { useHold } from "./useHold";
import type { ApprovalRequest } from "./types";

const meta: Meta = { title: "Approve card" };
export default meta;

/** The lane's width: the 288 px sidebar minus 8 px gutters. */
const W = 272;

function Card({ request, ...rest }: { request: ApprovalRequest } & Partial<Parameters<typeof ApproveCard>[0]>) {
  return (
    <div style={{ inlineSize: W }}>
      <ApproveCard request={request} {...rest} />
    </div>
  );
}
const one = (label: string, request: ApprovalRequest, extra: Partial<Parameters<typeof ApproveCard>[0]> = {}): StoryObj => ({
  render: () => (
    <ThemePair label={label}>
      <Card request={request} {...extra} />
    </ThemePair>
  ),
});

export const SafeBash: StoryObj = one("Safe command", APPROVALS.safeBash);
export const RiskyBash: StoryObj = one("Risky command, Allow is a hold", APPROVALS.riskyBash);
export const EditWithDiff: StoryObj = one("Edit with the first hunk", APPROVALS.edit);
export const WriteNewFile: StoryObj = one("Write of a new file", APPROVALS.writeNew);
export const McpTool: StoryObj = one("MCP tool, arguments as key: value", APPROVALS.mcp);
export const LongCommand: StoryObj = {
  render: () => (
    <ThemePair label="A 40-line command, collapsed then expanded">
      <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
        <Card request={APPROVALS.longCommand} />
        <Card request={APPROVALS.longCommand} expanded />
      </div>
    </ThemePair>
  ),
};
export const QuestionCard: StoryObj = one("Question", APPROVALS.question);
export const AfterDecision: StoryObj = {
  render: () => (
    <ThemePair label="After a decision">
      <div style={{ display: "grid", gap: "var(--space-2)", justifyItems: "start" }}>
        <Card request={APPROVALS.receipt} />
        <Card request={{ ...APPROVALS.receipt, state: { phase: "decided", verdict: "denied" } }} />
      </div>
    </ThemePair>
  ),
};
export const AnsweredInTerminal: StoryObj = one("Answered in the terminal", APPROVALS.answeredInTerminal);
export const ReleasedOnTimeout: StoryObj = one("Released on timeout", APPROVALS.released);
export const BeforeArming: StoryObj = one("First 500 ms: every action is inert", APPROVALS.safeBash, { armed: false });
export const HoldInProgress: StoryObj = one("Holding Y on a risky request (50%)", APPROVALS.riskyBash, { holdProgress: 0.5 });

/** Try it: Tab to the card, wait 500 ms, then Y / N / A / T / Space. Enter jumps and never allows. On the risky one, hold Y. */
function Interactive({ initial }: { initial: ApprovalRequest }) {
  const [request, setRequest] = useState(initial);
  const [expanded, setExpanded] = useState(false);
  const [log, setLog] = useState("Focus the card, then press a key.");
  const { armed, rearm } = useArming(request.id);
  const hold = useHold(() => decide("allowed"));
  function decide(verdict: "allowed" | "denied") {
    setRequest((r) => ({ ...r, state: { phase: "decided", verdict } }));
    setLog(`Decided: ${verdict}`);
  }
  return (
    <div style={{ inlineSize: W, display: "grid", gap: "var(--space-2)" }}>
      <ApproveCard
        request={request}
        expanded={expanded}
        armed={armed}
        holdProgress={hold.progress}
        onFocus={rearm}
        onHoldEnd={hold.cancel}
        onAction={(a) => {
          if (a === "allow") decide("allowed");
          else if (a === "deny") decide("denied");
          else if (a === "hold-allow") hold.begin();
          else if (a === "expand") setExpanded(true);
          else if (a === "terminal") setRequest((r) => ({ ...r, state: { phase: "terminal" } }));
          else setLog(`Action: ${a}`);
        }}
      />
      <p style={{ margin: 0, color: "var(--text-3)", fontSize: "var(--text-11)" }}>{log}</p>
    </div>
  );
}
export const InteractiveSafe: StoryObj = { render: () => <Interactive initial={APPROVALS.safeBash} /> };
export const InteractiveRisky: StoryObj = { render: () => <Interactive initial={APPROVALS.riskyBash} /> };
