import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { FIXTURES } from "../sidebar/fixtures";
import { SCENARIO_PLAN, SCENARIO_PORTS } from "../window/scenario";
import { Composer } from "./Composer";
import { StatusBar } from "./StatusBar";

const meta: Meta = { title: "Prompt bar", parameters: { layout: "fullscreen" } };
export default meta;

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;

/** A stand-in for the terminal above the line, so the flat bar is seen docked to an edge, as it is in the window. */
const terminal = (height: number) => (
  <div style={{ blockSize: height, padding: 12, background: "var(--bg-base)", color: "var(--text-2)", fontFamily: "var(--font-mono)", fontSize: 13, lineHeight: "20px", overflow: "clip" }}>
    <div>&gt; fix the flaky retry test in billing</div>
    <div>12 passed, 0 failed</div>
  </div>
);

/** The full prompt line as it sits under the terminal of a single-session window: the whole width, flat on the edge. */
const story = (label: string, node: React.ReactNode, width = 900): StoryObj => ({
  render: () => (
    <ThemePair label={label} stack>
      <div style={{ inlineSize: width, maxInlineSize: "100%", background: "var(--bg-base)" }}>
        {terminal(56)}
        {node}
      </div>
    </ThemePair>
  ),
});

/** The compact line as it sits inside one pane of a split view: a pane header, the terminal, the line. */
const paneStory = (label: string, name: string, node: React.ReactNode, width = 520): StoryObj => ({
  render: () => (
    <ThemePair label={label} stack>
      <div style={{ inlineSize: width, maxInlineSize: "100%", background: "var(--bg-base)" }}>
        <div style={{ display: "flex", alignItems: "center", blockSize: 28, paddingInline: 12, fontSize: 12, fontWeight: 600, color: "var(--text-1)" }}>{name}</div>
        {terminal(72)}
        {node}
      </div>
    </ThemePair>
  ),
});

export const Empty = story("Empty: a command line. The prompt glyph and the 2 px edge on top take the session's colour, the field is in the terminal's own font, and Send waits for text", <Composer row={row("idle")} />);
export const Typing = story("Typing: Send's Enter key lights in the vendor's colour", <Composer row={row("done-unseen")} value="Add retry backoff tests for billing and run them" />);
export const FourLinePrompt = story(
  "A four-line prompt: the field grows with the text, up to eight lines, in the window it grows upward over the terminal so the terminal never resizes",
  <Composer row={row("done-seen")} value={"Refactor the retry helper so the backoff uses the injected clock.\n- keep the public signature\n- add a test for the 3rd attempt\n- run npm test when you are done"} />,
);
export const WorkingQueued = story("The session is working: the reason sits in the row, the button says Queue, and the height does not change", <Composer row={row("working")} value="Also add tests for the refresh path" />);
export const WaitingPermission = story("Waiting on an approval: the field is off so a typed line can never be read as the answer; the placeholder says where to answer", <Composer row={row("needs-permission")} />);
export const WaitingQuestion = story("Waiting on a question: same, and the notice names a question", <Composer row={row("needs-question")} />);
export const Ended = story("Ended: the field is off and the way back is Resume", <Composer row={row("ended")} />);
export const SplitFocusedPane = story("Full line naming the focused pane (Pane 2 of 4) and taking that session's colour", <Composer row={row("idle")} pane={{ index: 1, count: 4 }} />);
export const EmptyPane = story("The focused pane is empty: nothing to write to yet", <Composer row={null} pane={{ index: 2, count: 3 }} />);
export const Narrow = story("At 560 px the hint hides and the target and tools keep their place", <Composer row={row("working")} value="x" pane={{ index: 0, count: 2 }} />, 560);

export const CompactEmpty = paneStory("Split view: every pane has its own line. One row: prompt glyph, field, Send. The pane that is not focused keeps a hairline edge", "docs-site", <Composer row={row("idle")} variant="compact" />);
export const CompactFocused = paneStory("The focused pane's line has the vendor-coloured edge on top, so you can see which prompt a keystroke will reach", "api-server", <Composer row={row("done-unseen")} variant="compact" focused value="Add retry backoff tests" />);
export const CompactWorking = paneStory("A working session in a pane: the notice stays one row, Send reads Queue", "api-server", <Composer row={row("working")} variant="compact" value="Also add tests" />, 640);
export const CompactWaiting = paneStory("A pane waiting on an approval: its own line is off, with the reason as the placeholder", "billing", <Composer row={row("needs-permission")} variant="compact" />);
export const CompactEnded = paneStory("An ended session's pane offers Resume in place of Send", "scratch", <Composer row={row("ended")} variant="compact" />, 640);
export const CompactNarrow = paneStory("A narrow pane (360 px, a quarter of a window): the notice keeps its glyph and the line still fits", "api-server", <Composer row={row("working")} variant="compact" value="x" />, 360);

const data = { plan: SCENARIO_PLAN, ports: SCENARIO_PORTS } as const;
/** The status bar under everything, at the width of the window's centre column. */
const barStory = (label: string, node: React.ReactNode, width = 1160): StoryObj => ({
  render: () => (
    <ThemePair label={label} stack>
      <div style={{ inlineSize: width, maxInlineSize: "100%", background: "var(--bg-base)" }}>
        {terminal(40)}
        {node}
      </div>
    </ThemePair>
  ),
});

export const StatusBarFull = barStory(
  "The status bar, wide: it starts with the session's number and name, so the facts always say whose they are; then model and effort, context with its bar and tokens, the 5-hour and 7-day limits with when they reset, cost, session time, uncommitted changes, MCP health, version, skills, environment and ports. It is one row for the focused session, whatever the layout",
  <StatusBar row={row("working")} {...data} />,
);
export const StatusBarNarrower = barStory(
  "At 700 px the least important items go first (environment, version, changes, skills, ports), then the healthy MCP count; the meters stay",
  <StatusBar row={row("working")} {...data} />,
  700,
);
export const StatusBarNarrow = barStory(
  "At 460 px only the essentials stay: model, context, the 5-hour limit and the 7-day percentage, each shortened. Nothing wraps and nothing is cut mid-word",
  <StatusBar row={row("working")} {...data} />,
  460,
);
export const StatusBarMcpProblem = barStory(
  "An MCP server is failing: its name replaces the healthy count, with a caution mark, and it outlasts everything but the core meters",
  <StatusBar row={row("needs-permission")} {...data} />,
);
export const StatusBarNothingKnown = barStory("Nothing known about the session: the bar is an empty row of the same height, never a row of dashes", <StatusBar row={row("tiny")} />);
export const StatusBarEmptyPane = barStory("The focused pane is empty: the bar stays, empty, so the window never changes height when focus moves", <StatusBar row={null} />);
export const StatusBarNearlyFull = barStory(
  "Context at 96% and the 5-hour limit at 88%: the figure goes bold and the bar changes colour (caution from 80%, error from 95%, the same steps as the context ring); the number is always written",
  <StatusBar
    row={{ ...row("working"), contextPct: 96, usage: { ...row("working").usage, inContext: 192_000 } }}
    plan={{ fiveHour: { pct: 88, resetsInMs: 1_500_000 }, sevenDay: { pct: 61, resetsInMs: 190_000_000 } }}
    ports={SCENARIO_PORTS}
  />,
);
export const StatusBarFittedInScript = barStory(
  "Fitted in script instead of by the stylesheet (for places that cannot measure): 70 characters wide, the same degrade order, never wrapped",
  <StatusBar row={row("working")} {...data} width={70} />,
  600,
);
