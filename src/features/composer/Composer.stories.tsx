import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { FIXTURES } from "../sidebar/fixtures";
import { Composer } from "./Composer";

const meta: Meta = { title: "Prompt bar", parameters: { layout: "fullscreen" } };
export default meta;

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;

/** The bar as it sits under the terminals: full width of the centre column, on the terminal's own surface. */
const frame = (children: React.ReactNode, width = 900) => (
  <div style={{ inlineSize: width, maxInlineSize: "100%", paddingBlockStart: 12, background: "var(--bg-base)" }}>{children}</div>
);

const story = (label: string, node: React.ReactNode, width?: number): StoryObj => ({
  render: () => <ThemePair label={label} stack>{frame(node, width)}</ThemePair>,
});

export const Empty = story("Empty: the placeholder names the session, Enter and Shift+Enter are spelled out, Send waits for text", <Composer row={row("idle")} />);
export const Typing = story("Typing: Send takes the vendor's colour", <Composer row={row("done-unseen")} value="Add retry backoff tests for billing and run them" />);
export const FourLinePrompt = story(
  "A four-line prompt: the field grows with the text, up to eight lines, and the bar below it does not move",
  <Composer
    row={row("done-seen")}
    value={"Refactor the retry helper so the backoff uses the injected clock.\n- keep the public signature\n- add a test for the 3rd attempt\n- run npm test when you are done"}
  />,
);
export const WorkingQueued = story("The session is working: the notice says the message is queued, the button says Queue, and the bar keeps its height", <Composer row={row("working")} value="Also add tests for the refresh path" />);
export const WaitingPermission = story("Waiting on an approval: the field is off so a typed line can never be read as the answer; the placeholder says where to answer", <Composer row={row("needs-permission")} />);
export const WaitingQuestion = story("Waiting on a question: same, and the notice names a question", <Composer row={row("needs-question")} />);
export const Ended = story("Ended: the field is off and the way back is Resume", <Composer row={row("ended")} />);
export const SplitFocusedPane = story("In split view the bar names the focused pane (Pane 2 of 4) and takes that session's colour", <Composer row={row("idle")} pane={{ index: 1, count: 4 }} />);
export const EmptyPane = story("The focused pane is empty: nothing to write to yet", <Composer row={null} pane={{ index: 2, count: 3 }} />);
export const Narrow = story("At 560 px the hint hides and the target and tools keep their place", <Composer row={row("working")} value="x" pane={{ index: 0, count: 2 }} />, 560);
