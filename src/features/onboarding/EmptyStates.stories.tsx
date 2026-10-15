import type { Meta, StoryObj } from "@storybook/react-vite";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { ThemePair } from "../../design/ThemePair";
import { EMPTY_STATES } from "./emptyStates";

const meta: Meta = { title: "Empty states", parameters: { layout: "fullscreen" } };
export default meta;

export const EmptyStates: StoryObj = {
  render: () => (
    <ThemePair label="Every empty state: what is empty, one sentence, and how to start" stack>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 8, padding: 12 }}>
        {EMPTY_STATES.map((e) => (
          <EmptyState key={e.id} title={e.title} body={e.body} action={e.action ? { label: e.action } : undefined} />
        ))}
      </div>
    </ThemePair>
  ),
};
