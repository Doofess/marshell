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
    <ThemePair label="Twenty sessions in an 800 px tall window: the list scrolls (wheel, touch, keys) with no scrollbar drawn, and fades at the edge that has more. The lane, by contrast, shows whole rows and a +N more button">
      <div style={{ blockSize: HEIGHT, display: "flex" }}>
        <Sidebar rows={twenty} requests={SCENARIO_REQUESTS} height={HEIGHT} width={288} selectedId="s0" footer={SCENARIO_FOOTER} />
      </div>
    </ThemePair>
  ),
};

export const RailWithTwentySessions: StoryObj = {
  render: () => (
    <ThemePair label="The same twenty in the 52 px rail: it scrolls the same way, with the palette pinned at the foot">
      <div style={{ blockSize: HEIGHT, display: "flex" }}>
        <Rail rows={twenty} needsYou={2} selectedId="s0" />
      </div>
    </ThemePair>
  ),
};

export const AllSessionsMenuOpen: StoryObj = {
  render: () => (
    <ThemePair label="Where the lane's +N more leads: every request or session in a menu. Menus keep a themed scrollbar, so this list shows one">
      <div style={{ padding: 16 }}>
        <AllSessionsMenu rows={twenty} selectedId="s0" />
      </div>
    </ThemePair>
  ),
};
