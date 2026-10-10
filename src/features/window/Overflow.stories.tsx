import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { FIXTURES } from "../sidebar/fixtures";
import { AllSessionsMenu } from "./AllSessionsMenu";
import { Rail } from "./Rail";
import { SCENARIO_FOOTER, SCENARIO_REQUESTS } from "./scenario";
import { Sidebar } from "./Sidebar";

const meta: Meta = { title: "Overflow", parameters: { layout: "fullscreen" } };
export default meta;

const KINDS = ["working", "idle", "done-unseen", "done-seen", "error", "needs-permission", "needs-question"].map((id) => FIXTURES.find((f) => f.id === id)!);
const twenty = Array.from({ length: 20 }, (_, i) => ({ ...KINDS[i % KINDS.length]!, id: `s${i}`, name: `${KINDS[i % KINDS.length]!.name}-${i + 1}` }));
const HEIGHT = 800;

export const SidebarWithTwentySessions: StoryObj = {
  render: () => (
    <ThemePair label="Twenty sessions in an 800 px tall window: the sidebar never scrolls. It shows the rows that fit and a +N more sessions button; the lane does the same for requests">
      <div style={{ blockSize: HEIGHT, display: "flex" }}>
        <Sidebar rows={twenty} requests={SCENARIO_REQUESTS} height={HEIGHT} width={288} selectedId="s0" footer={SCENARIO_FOOTER} />
      </div>
    </ThemePair>
  ),
};

export const RailWithTwentySessions: StoryObj = {
  render: () => (
    <ThemePair label="The same twenty in the 52 px rail: the sessions that fit, then +N. The palette stays at the foot">
      <div style={{ blockSize: HEIGHT, display: "flex" }}>
        <Rail rows={twenty} needsYou={2} selectedId="s0" height={HEIGHT} />
      </div>
    </ThemePair>
  ),
};

export const AllSessionsMenuOpen: StoryObj = {
  render: () => (
    <ThemePair label="Where +N more leads: every session in a menu. Menus may scroll, so this list does, with the themed scrollbar showing">
      <div style={{ padding: 16 }}>
        <AllSessionsMenu rows={twenty} selectedId="s0" />
      </div>
    </ThemePair>
  ),
};
