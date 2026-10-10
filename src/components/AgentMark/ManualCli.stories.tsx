import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { FIXTURES } from "../../features/sidebar/fixtures";
import { SessionRow } from "../../features/sidebar/SessionRow";
import { SESSION_NUMBER_HINT, nextSessionNumber } from "../../features/sidebar/sessionNumber";
import { SCENARIO_ROWS_WITH_TOOLS } from "../../features/window/scenario";
import { SessionNumber } from "../SessionNumber/SessionNumber";
import { AgentMark } from "./AgentMark";
import { TOOL_GROUPS, TOOL_GROUP, TOOL_GROUP_NAMES, TOOL_IDS, TOOL_NAMES } from "./tools";

const meta: Meta = { title: "Manual CLIs and session numbers" };
export default meta;

const grid = { display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(104px, 1fr))", gap: "var(--space-4)" } as const;
const cell = { display: "grid", justifyItems: "center", gap: "var(--space-2)", fontSize: "var(--text-11)", color: "var(--text-2)", textAlign: "center", margin: 0 } as const;
const heading = { margin: "0 0 var(--space-2)", fontSize: "var(--text-12)", color: "var(--text-3)", fontWeight: 500 } as const;

/** Every program Marshell recognises, in its group, at the three sizes the app draws a mark. They are drawn here as stand-ins; the programs' own logos get sourced before release. */
export const ToolMarks: StoryObj = {
  render: () => (
    <ThemePair label="Manual CLI marks: shells, dev tools, remote and cloud, anything else" stack>
      <div style={{ display: "grid", gap: "var(--space-5)" }}>
        {TOOL_GROUPS.map((group) => (
          <div key={group}>
            <h4 style={heading}>{TOOL_GROUP_NAMES[group]}</h4>
            <div style={grid}>
              {TOOL_IDS.filter((id) => TOOL_GROUP[id] === group).map((id) => (
                <figure key={id} style={cell}>
                  <span style={{ display: "flex", alignItems: "center", gap: "var(--space-3)" }}>
                    <AgentMark agent={id} size={12} />
                    <AgentMark agent={id} size={16} />
                    <AgentMark agent={id} size={20} />
                  </span>
                  <figcaption>{TOOL_NAMES[id]}</figcaption>
                </figure>
              ))}
            </div>
          </div>
        ))}
      </div>
    </ThemePair>
  ),
};

/** An AI agent next to the plain terminals: the agent's mark moves while it works, a plain terminal's never does. */
export const StillNotMoving: StoryObj = {
  render: () => (
    <ThemePair label="A working agent and a running command">
      <div style={{ display: "flex", gap: "var(--space-6)", alignItems: "center", color: "var(--text-2)", fontSize: "var(--text-12)" }}>
        <figure style={cell}>
          <AgentMark agent="claude" size={20} working />
          <figcaption>Claude, working: moves</figcaption>
        </figure>
        <figure style={cell}>
          <AgentMark agent="docker" size={20} working />
          <figcaption>Docker, running a command: still</figcaption>
        </figure>
        <figure style={cell}>
          <AgentMark agent="powershell" size={20} />
          <figcaption>PowerShell, at the prompt: still</figcaption>
        </figure>
      </div>
    </ThemePair>
  ),
};

function Column({ ids, density }: { ids: string[]; density: "comfortable" | "expanded" }) {
  const rows = ids.map((id) => FIXTURES.find((f) => f.id === id)!);
  return (
    <div role="listbox" aria-label="Sessions" style={{ inlineSize: 288, background: "var(--bg-raised)" }}>
      {rows.map((r, i) => (
        <SessionRow key={r.id} row={r} density={density} selected={r.id === "tool-docker"} tabStop={i === 0} />
      ))}
    </div>
  );
}

const MIXED = ["working", "tool-powershell", "tool-docker", "needs-permission", "tool-ssh"];
/** Plain terminals sit in the same list as the agents: a mark, a number, a name, what they are doing. No ring, model, mode or motion. */
export const RowsComfortable: StoryObj = {
  render: () => (
    <ThemePair label="Agents and manual CLIs in the same list (comfortable)">
      <Column ids={MIXED} density="comfortable" />
    </ThemePair>
  ),
};
export const RowsExpanded: StoryObj = {
  render: () => (
    <ThemePair label="The same rows, expanded: a plain terminal has no second line of model, effort, recap or usage to show">
      <Column ids={MIXED} density="expanded" />
    </ThemePair>
  ),
};

/** Numbers stay with their sessions: the agents hold 1, 2, 3, 5 and 6 (session 4 was closed) and the three manual sessions 7, 8 and 9. The next one started takes 4. */
export const NumberingGap: StoryObj = {
  render: () => {
    const taken = SCENARIO_ROWS_WITH_TOOLS.map((r) => r.number);
    const next = nextSessionNumber(taken);
    return (
      <ThemePair label="Stable numbers: a closed session's number is the next one used">
        <div style={{ display: "grid", gap: "var(--space-3)", fontSize: "var(--text-12)", color: "var(--text-2)" }}>
          <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center", flexWrap: "wrap" }}>
            {Array.from({ length: 10 }, (_, i) => i + 1).map((n) => {
              const r = SCENARIO_ROWS_WITH_TOOLS.find((x) => x.number === n);
              return (
                <span key={n} style={{ display: "inline-flex", alignItems: "center", gap: "var(--space-1)", color: r ? undefined : "var(--text-3)", fontStyle: r ? undefined : "italic" }}>
                  <SessionNumber n={n} />
                  <span>{r ? r.name : n === next ? "next session" : "free"}</span>
                </span>
              );
            })}
          </div>
          <p style={{ margin: 0 }}>
            A session keeps its number until it closes, and a new one takes the lowest free number, so "session 6" means one terminal for as long as it lives. Ctrl+1 to Ctrl+9 go to sessions 1 to 9; any other number is "Go to session 12" in the palette.
          </p>
          <p style={{ margin: 0 }}>{SESSION_NUMBER_HINT}.</p>
        </div>
      </ThemePair>
    );
  },
};
