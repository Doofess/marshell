import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ThemePair } from "../../design/ThemePair";
import { COMMANDS } from "../../lib/keymap";
import { filterCommands, filterSessions, parseSessionNumber } from "../launcher/filter";
import { PALETTE_SESSIONS } from "./fixtures";
import { Palette } from "./Palette";

const meta: Meta = { title: "Command palette", parameters: { layout: "fullscreen" } };
export default meta;

const frameStyle = { blockSize: 520, position: "relative" as const, background: "var(--bg-base)", borderRadius: 8, overflow: "clip" as const };
const frame = (query: string, activeIndex = 0, os: "windows" | "mac" = "windows", withSessions = true) => (
  <div style={frameStyle}>
    <Palette view={{ query, activeIndex }} sessions={withSessions ? PALETTE_SESSIONS : undefined} os={os} />
  </div>
);

export const Empty: StoryObj = { render: () => <ThemePair label="Empty: open sessions with their numbers first, then every command, grouped, with its shortcut" stack>{frame("")}</ThemePair> };
export const SessionSix: StoryObj = { render: () => <ThemePair label={'Typing "6": session 6 comes first, whatever it runs'} stack>{frame("6")}</ThemePair> };
export const SessionByHashAndWords: StoryObj = { render: () => <ThemePair label={'"#7" and "session 7" mean the same: a docker build, which is a plain terminal'} stack>{frame("session 7")}</ThemePair> };
export const SessionPastNine: StoryObj = { render: () => <ThemePair label={'"12": sessions past nine have no Ctrl+digit, so this is how you reach them'} stack>{frame("#12")}</ThemePair> };
export const NoSuchSession: StoryObj = { render: () => <ThemePair label={'"#4": nobody holds 4, so it says which numbers exist'} stack>{frame("#4")}</ThemePair> };
export const SessionByName: StoryObj = { render: () => <ThemePair label={'Filtering by a name or folder: "my-app" finds the sessions that run there'} stack>{frame("my-app")}</ThemePair> };
export const Filtering: StoryObj = { render: () => <ThemePair label={'Filtering: "split"'} stack>{frame("split")}</ThemePair> };
export const ScatteredLetters: StoryObj = { render: () => <ThemePair label={'Scattered letters: "stse" finds Start a session'} stack>{frame("stse")}</ThemePair> };
export const NoMatch: StoryObj = { render: () => <ThemePair label="Nothing matches" stack>{frame("qqqq")}</ThemePair> };
export const MacShortcuts: StoryObj = { render: () => <ThemePair label="macOS shortcut labels: ⌘6 goes to session 6" stack>{frame("", 0, "mac")}</ThemePair> };
export const CommandsOnly: StoryObj = { render: () => <ThemePair label="With no sessions open it is the plain command list" stack>{frame("", 0, "windows", false)}</ThemePair> };

/** Try it: type a command, a name, or a session number; arrows move, Enter runs, Esc clears. */
function Live() {
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const [ran, setRan] = useState<string | null>(null);
  const size = filterSessions(q, PALETTE_SESSIONS).length + (parseSessionNumber(q) !== null ? 0 : filterCommands(q, COMMANDS.filter((c) => !c.id.startsWith("focus-session-"))).length);
  return (
    <div style={frameStyle}>
      <Palette
        view={{ query: q, activeIndex: Math.min(active, Math.max(0, size - 1)) }}
        sessions={PALETTE_SESSIONS}
        onQuery={(next) => {
          setQ(next);
          setActive(0);
        }}
        onActive={setActive}
        onRun={setRan}
        onClose={() => setQ("")}
      />
      <p style={{ position: "absolute", insetBlockEnd: 8, insetInlineStart: 12, margin: 0, color: "var(--text-2)", fontSize: "var(--text-12)" }}>{ran ? `Ran: ${ran}` : "Nothing run yet"}</p>
    </div>
  );
}
export const Interactive: StoryObj = { render: () => <Live /> };
