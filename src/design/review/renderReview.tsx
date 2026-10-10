import type { FC } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import * as Colours from "../Colours.stories";
import * as Type from "../Type.stories";
import * as SpaceAndMotion from "../SpaceAndMotion.stories";
import * as Glyphs from "../../components/StatusGlyph/StatusGlyph.stories";
import * as Marks from "../../components/AgentMark/AgentMark.stories";
import * as Card from "../../features/approval/ApproveCard.stories";
import * as Lane from "../../features/lane/Lane.stories";
import * as Window from "../../features/window/MainWindow.stories";
import * as Icon from "../../components/Logo/Logo.stories";
import * as Overflow from "../../features/window/Overflow.stories";
import * as KeyboardNav from "../../features/window/NavCursor.stories";
import * as PromptBar from "../../features/composer/Composer.stories";
import * as ManualCli from "../../components/AgentMark/ManualCli.stories";
import * as Launcher from "../../features/launcher/Launcher.stories";
import * as Palette from "../../features/palette/Palette.stories";
import * as Doctor from "../../features/doctor/Doctor.stories";
import * as Settings from "../../features/settings/Settings.stories";
import * as Onboarding from "../../features/onboarding/Onboarding.stories";
import * as EmptyStates from "../../features/onboarding/EmptyStates.stories";
import * as DiffStories from "../../components/DiffView/DiffView.stories";
import * as Rows from "../../features/sidebar/SessionRow.stories";

type StoryModule = { default: { title?: string } } & Record<string, unknown>;

/** The stories that make up one review batch, in reading order. */
const BATCH_1: Array<[StoryModule, string]> = [
  [Colours, "Every colour pairing is checked live against the AA and distinctness floors; red cells would mark a failure."],
  [Type, "The 11–28 px scale with real copy, tabular figures and the mono stack."],
  [SpaceAndMotion, "Spacing steps and the motion durations every animation uses."],
  [Glyphs, "Ten states plus muted, administrator and caution, at 16 and 12 px, with their reduced-motion forms."],
  [Marks, "The vendor marks, how each one moves while its session works, and the status glyphs and context ring in each vendor's colour."],
  [Rows, "Every row state in each density, and the truncation order down to the 200 px minimum sidebar."],
];

const BATCH_2A: Array<[StoryModule, string]> = [
  [Card, "The card in every state from the plan: safe and risky commands, edit with diff, new file, MCP, a 40-line command, a question, the receipt, answered in terminal and released on timeout. Interactive stories need Storybook."],
];

export type UpTo = "1" | "2a" | "2b" | "2c" | "3a" | "3b" | "3c";
const ORDER: UpTo[] = ["1", "2a", "2b", "2c", "3a", "3b", "3c"];
const BATCHES: Record<UpTo, Array<[StoryModule, string]>> = {
  "1": BATCH_1,
  "2a": BATCH_2A,
  "2b": [
    [Lane, "The lane header is always there at 28 px. The oldest waiting request is the target and shows as a full card; the rest are 36 px one-liners; it never takes more than 40% of the sidebar."],
    [Icon, "The app icon, and where it lives: the first thing in the window's top bar, where it opens the app menu."],
    [PromptBar, "The prompt line is a flat command line docked under the terminal, not a floating card. A single terminal has one full line; in split view every pane has its own compact line, so a line of text always goes to the session of its own pane. It takes that session's colour, is off while the session waits on you or has ended, and never changes height: notices sit in the line, so the terminals above do not resize when a session starts working. The strip of status data (model, context, plan limits, cost, MCP and more) lives in a thin status bar at the foot of the window, for the focused session."],
    [ManualCli, "Terminals are not only AI agents. A manual CLI (a shell, a dev tool, a remote or cloud tool) gets its program's mark the same way an agent gets its vendor's, but it is a plain terminal: no thinking motion, no context ring, no model, no prompt line of ours. Every open terminal also has a number that stays with it until it closes, so you can tell an agent \"go talk to session 6\"."],
    [KeyboardNav, "Terminals are selected, and the keyboard moves the selection without ever taking a key the terminal program needs. Hold Ctrl+Shift (⌃⇧ on macOS) and tap Tab to select the next terminal on screen; hold it and use the arrows to move a cursor anywhere, including down the sidebar list, and release to select. Plain Tab, Esc and the arrows still go to the program. Backspace with the chord, or Esc away from a terminal, selects none. Shortcuts are matched by physical key and none uses punctuation, so they work on any keyboard layout; the cheat sheets show the audit result for each system."],
    [Overflow, "The session list in the sidebar and rail scrolls with no visible scrollbar, and a soft fade shows where more rows continue. The needs-you lane never scrolls: it shows what fits and a \"+N more\" button that opens a menu, and menus, unlike the window, keep a themed scrollbar."],
    [Window, "The window in the real layout, with a real xterm.js terminal that follows the theme. The accent follows the session's vendor. Split view shows up to four terminals at once and picks the arrangement that suits the window. Use the Terminal theme control above to pin the terminal dark or light, and Contrast protection to see what it rescues."],
  ],
  "2c": [
    [Launcher, "One sentence of chips and a prompt line, 560 px wide, 20% from the top. The first chip lists agents, shells and tools. A shell or tool has no mode, pace or worktree: it is a plain terminal, so the line asks for a first command. The footer says which session number it will take."],
    [Palette, "Open sessions come first with their number, mark, name and status, and the number is the fastest way in: type 6, #6 or session 6. A number nobody holds says which sessions are open. Commands follow, with their shortcuts."],
  ],
  "3a": [
    [Onboarding, "Four screens, under 60 seconds: what was found, what Marshell may change (shown as the exact diff, with the backup and the way out), notifications with a preview of each sound, and the first session. Nothing is changed without a yes, and a file that does not parse is never edited."],
    [EmptyStates, "Every empty state says what is empty and how to start."],
    [DiffStories, "The one diff view used for consent, the doctor's fixes and settings changes."],
  ],
  "3b": [
    [Doctor, "The report, the fix sheet with the combined diff grouped by file, the receipt toast with Restart them and Undo (10 seconds), and Undo last fix in the header afterwards. Problems Marshell cannot fix say why, and managed settings are listed, never changed."],
    [Settings, "Form and raw editing of each CLI's settings, with a scope badge on every value (User, Project, Local, Managed, Marshell). Every change is shown as the exact diff, with the backup and the way to undo, before Apply. A file that does not parse, is read-only, or changed on disk is never overwritten."],
  ],
  "3c": [],
};

/** "TruncationAt200" → "Truncation at 200". */
function humanize(name: string): string {
  const words = name.replace(/([a-z])([A-Z0-9])/g, "$1 $2").replace(/([0-9])([A-Z])/g, "$1 $2");
  return words.charAt(0) + words.slice(1).toLowerCase();
}

function isStory(v: unknown): v is { render: FC } {
  return typeof v === "object" && v !== null && typeof (v as { render?: unknown }).render === "function";
}

/** Server-renders every story up to and including the given batch into page sections. */
export function renderReview(upTo: UpTo = "2a"): string {
  // Each story is rendered on its own, so React would hand every one the same ids (and the same radio-group names); a prefix keeps them apart.
  let storyNo = 0;
  const modules = ORDER.slice(0, ORDER.indexOf(upTo) + 1).flatMap((k) => BATCHES[k]);
  return modules.map(([mod, blurb]) => {
    const title = mod.default.title ?? "Untitled";
    const stories = Object.entries(mod).filter(([k, v]) => k !== "default" && isStory(v)) as Array<[string, { render: FC }]>;
    const id = title.toLowerCase().replace(/[^a-z0-9]+/g, "-");
    const body = stories
      .map(([name, story]) => {
        const Render = story.render;
        return `<article class="review__story"><h3>${humanize(name)}</h3><div class="review__canvas">${renderToStaticMarkup(<Render />, { identifierPrefix: `s${++storyNo}-` })}</div></article>`;
      })
      .join("");
    return `<section class="review__section" id="${id}"><h2>${title.replace("/", " / ")}</h2><p class="review__blurb">${blurb}</p>${body}</section>`;
  }).join("");
}
