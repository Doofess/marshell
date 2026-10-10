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

export type UpTo = "1" | "2a" | "2b" | "2c";
const ORDER: UpTo[] = ["1", "2a", "2b", "2c"];
const BATCHES: Record<UpTo, Array<[StoryModule, string]>> = {
  "1": BATCH_1,
  "2a": BATCH_2A,
  "2b": [
    [Lane, "The lane header is always there at 28 px. The oldest waiting request is the target and shows as a full card; the rest are 36 px one-liners; it never takes more than 40% of the sidebar."],
    [Window, "The 5-agent scenario in the real layout, with a real xterm.js terminal that follows the theme. Use the Terminal theme control above to pin it dark or light, and Contrast protection to see what it rescues."],
  ],
  "2c": [],
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
  const modules = ORDER.slice(0, ORDER.indexOf(upTo) + 1).flatMap((k) => BATCHES[k]);
  return modules.map(([mod, blurb]) => {
    const title = mod.default.title ?? "Untitled";
    const stories = Object.entries(mod).filter(([k, v]) => k !== "default" && isStory(v)) as Array<[string, { render: FC }]>;
    const id = title.toLowerCase().replace(/[^a-z0-9]+/g, "-");
    const body = stories
      .map(([name, story]) => {
        const Render = story.render;
        return `<article class="review__story"><h3>${humanize(name)}</h3><div class="review__canvas">${renderToStaticMarkup(<Render />)}</div></article>`;
      })
      .join("");
    return `<section class="review__section" id="${id}"><h2>${title.replace("/", " / ")}</h2><p class="review__blurb">${blurb}</p>${body}</section>`;
  }).join("");
}
