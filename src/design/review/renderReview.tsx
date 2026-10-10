import type { FC } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import * as Colours from "../Colours.stories";
import * as Type from "../Type.stories";
import * as SpaceAndMotion from "../SpaceAndMotion.stories";
import * as Glyphs from "../../components/StatusGlyph/StatusGlyph.stories";
import * as Marks from "../../components/AgentMark/AgentMark.stories";
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

/** "TruncationAt200" → "Truncation at 200". */
function humanize(name: string): string {
  const words = name.replace(/([a-z])([A-Z0-9])/g, "$1 $2").replace(/([0-9])([A-Z])/g, "$1 $2");
  return words.charAt(0) + words.slice(1).toLowerCase();
}

function isStory(v: unknown): v is { render: FC } {
  return typeof v === "object" && v !== null && typeof (v as { render?: unknown }).render === "function";
}

/** Server-renders every story of the batch into page sections. */
export function renderReview(): string {
  return BATCH_1.map(([mod, blurb]) => {
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
