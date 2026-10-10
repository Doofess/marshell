import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { APPROVALS } from "../approval/fixtures";
import { Lane } from "./Lane";

const meta: Meta = { title: "Needs-you lane" };
export default meta;

const frame = (children: React.ReactNode) => (
  <div style={{ inlineSize: 288, background: "var(--bg-raised)", borderInlineEnd: "1px solid var(--hairline)" }}>{children}</div>
);

export const AllClear: StoryObj = { render: () => <ThemePair label="All clear">{frame(<Lane requests={[]} sidebarHeight={800} />)}</ThemePair> };
export const OneWaiting: StoryObj = { render: () => <ThemePair label="One waiting">{frame(<Lane requests={[APPROVALS.safeBash]} sidebarHeight={800} />)}</ThemePair> };
export const TwoWaiting: StoryObj = {
  render: () => <ThemePair label="Two waiting: the oldest is expanded">{frame(<Lane requests={[APPROVALS.safeBash, APPROVALS.question]} sidebarHeight={800} />)}</ThemePair>,
};
const many = Array.from({ length: 12 }, (_, i) => ({ ...APPROVALS.safeBash, id: `r${i}`, session: { ...APPROVALS.safeBash.session, name: `session-${i + 1}` }, waitingMs: 60_000 * (i + 1) }));
export const TwelveWaiting: StoryObj = { render: () => <ThemePair label="Twelve waiting in a 520 px sidebar: scrolls, +N more">{frame(<Lane requests={many} sidebarHeight={520} />)}</ThemePair> };
