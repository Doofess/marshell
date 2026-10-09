import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { FIXTURES } from "./fixtures";
import { SessionRow } from "./SessionRow";
import type { Density } from "./types";

const meta: Meta = { title: "Sidebar/Session rows" };
export default meta;

function Column({ density, width, ids }: { density: Density; width: number; ids?: string[] }) {
  const rows = ids ? FIXTURES.filter((f) => ids.includes(f.id)) : FIXTURES;
  return (
    <div role="listbox" aria-label="Sessions" style={{ inlineSize: width, background: "var(--bg-raised)", borderInlineEnd: "1px solid var(--hairline)" }}>
      {rows.map((r, i) => (
        <SessionRow key={r.id} row={r} density={density} selected={r.id === "working"} tabStop={rows.some((x) => x.id === "working") ? r.id === "working" : i === 0} />
      ))}
    </div>
  );
}

export const Compact: StoryObj = {
  render: () => (
    <ThemePair label="Compact rows at 264 px">
      <Column density="compact" width={264} />
    </ThemePair>
  ),
};

export const Comfortable: StoryObj = {
  render: () => (
    <ThemePair label="Comfortable rows at 264 px">
      <Column density="comfortable" width={264} />
    </ThemePair>
  ),
};

export const Expanded: StoryObj = {
  render: () => (
    <ThemePair label="Expanded rows at 264 px">
      <Column density="expanded" width={264} ids={["working", "needs-permission", "bypass", "unknown", "error"]} />
    </ThemePair>
  ),
};

/** The minimum sidebar width. Check the truncation order: branch (middle), then project, then name (8 characters kept). */
export const TruncationAt200: StoryObj = {
  render: () => (
    <ThemePair label="Truncation at 200 px">
      <Column density="comfortable" width={200} ids={["long-names", "crowded", "emoji-name", "cjk-emoji", "rtl", "tiny", "bypass", "muted-needs-you", "needs-question"]} />
    </ThemePair>
  ),
};

/** The same long row at shrinking widths, to see each truncation step. */
export const TruncationSteps: StoryObj = {
  render: () => (
    <ThemePair label="Truncation steps">
      {[400, 330, 280, 240, 200].map((w) => (
        <div key={w} style={{ marginBlockEnd: "var(--space-2)" }}>
          <div style={{ fontSize: "var(--text-11)", color: "var(--text-3)" }}>{w} px</div>
          <Column density="compact" width={w} ids={["long-names"]} />
        </div>
      ))}
    </ThemePair>
  ),
};
