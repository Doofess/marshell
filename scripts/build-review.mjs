// Builds one self-contained design-review page from the batch's Storybook stories.
// Usage: node scripts/build-review.mjs <out.html>
// The stories are server-rendered with Vite's SSR loader; the CSS is the shipped token and component CSS, inlined.
import { readFileSync, writeFileSync } from "node:fs";
import { createServer } from "vite";

const out = process.argv[2];
if (!out) {
  console.error("usage: node scripts/build-review.mjs <out.html>");
  process.exit(1);
}

const CSS = [
  "src/styles/tokens.css",
  "src/styles/accents.css",
  "src/styles/base.css",
  "src/styles/motion.css",
  "src/design/ThemePair.css",
  "src/design/specimen.css",
  "src/components/AgentMark/agent-colors.css",
  "src/components/AgentMark/AgentMark.css",
  "src/components/StatusGlyph/StatusGlyph.css",
  "src/components/ContextRing/ContextRing.css",
  "src/features/sidebar/SessionRow.css",
];
const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"];

const server = await createServer({ appType: "custom", server: { middlewareMode: true }, logLevel: "error" });
let sections;
try {
  const { renderReview } = await server.ssrLoadModule("/src/design/review/renderReview.tsx");
  sections = renderReview();
} finally {
  await server.close();
}

const css = CSS.map((f) => `/* ${f} */\n${readFileSync(f, "utf8")}`).join("\n");

// Page chrome. It uses the app's own tokens, which follow the viewer's theme through light-dark().
const chrome = `
.review {
  display: grid;
  gap: var(--space-8);
  max-inline-size: 1440px;
  margin-inline: auto;
  padding-inline: 16px;
  padding-block: var(--space-6) var(--space-8);
}
.review h1 {
  margin: 0;
  font-size: var(--text-28);
  font-weight: 600;
  text-wrap: balance;
}
.review__lede {
  margin: var(--space-2) 0 0;
  max-inline-size: 65ch;
  color: var(--text-2);
  font-size: var(--text-15);
}
.review__bar {
  position: sticky;
  inset-block-start: env(safe-area-inset-top, 0px);
  z-index: 1;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2) var(--space-4);
  padding-block: var(--space-2);
  background: var(--bg-base);
  border-block-end: 1px solid var(--hairline);
  font-size: var(--text-12);
  color: var(--text-2);
}
.review__bar label {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}
.review__bar :is(select, button, input) {
  font: inherit;
  color: var(--text-1);
}
.review__bar :is(select, button) {
  padding-block: 2px;
  padding-inline: var(--space-2);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-raised);
}
.review__bar button:active {
  background: var(--selected);
}
.review__bar nav {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  margin-inline-start: auto;
}
.review__bar a {
  color: var(--text-2);
}
.review__section {
  display: grid;
  gap: var(--space-4);
  scroll-margin-block-start: 64px;
}
.review__section h2 {
  margin: 0;
  font-size: var(--text-20);
  font-weight: 600;
}
.review__blurb {
  margin: 0;
  max-inline-size: 65ch;
  color: var(--text-2);
}
.review__story {
  display: grid;
  gap: var(--space-2);
  min-inline-size: 0;
}
.review__story > h3 {
  margin: 0;
  color: var(--text-3);
  font-size: var(--text-11);
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.review__canvas {
  min-inline-size: 0;
  overflow-x: auto;
}
.review__check {
  display: grid;
  gap: var(--space-2);
  max-inline-size: 70ch;
  margin: 0;
  padding-inline-start: var(--space-6);
}
@media (hover: hover) and (pointer: fine) {
  .review__bar a:hover {
    color: var(--text-1);
  }
}
`;

const options = ACCENTS.map((a) => `<option value="${a}">${a[0].toUpperCase()}${a.slice(1)}</option>`).join("");

const html = `<title>Marshell design review</title>
<style>
${css}
${chrome}
</style>
<main class="review">
  <header>
    <h1>Marshell design review</h1>
    <p class="review__lede">Batch 1 of 3: tokens, status glyphs and sidebar rows. Batch 2 (approve card, main window, launcher) builds on what is signed off here.</p>
  </header>
  <div class="review__bar">
    <label for="accent">Accent <select id="accent">${options}</select></label>
    <label for="motion"><input type="checkbox" id="motion"> Reduced motion</label>
    <button type="button" id="replay">Replay animations</button>
    <nav aria-label="Sections">
      <a href="#design-colours">Colours</a>
      <a href="#design-type">Type</a>
      <a href="#design-space-and-motion">Space</a>
      <a href="#components-status-glyphs">Glyphs</a>
      <a href="#agent-marks">Marks</a>
      <a href="#sidebar-session-rows">Rows</a>
      <a href="#sign-off">Sign-off</a>
    </nav>
  </div>
  ${sections}
  <section class="review__section" id="sign-off">
    <h2>Sign-off checklist</h2>
    <ol class="review__check">
      <li>The colour matrices show no red cells, and amber, caution, error and Claude orange read as four different things.</li>
      <li>Each of the ten states is recognisable at 12 px without relying on colour, and the reduced-motion forms make sense.</li>
      <li>Compact, comfortable and expanded rows read well, and truncation holds at 200 px, including CJK, emoji and Arabic names.</li>
      <li>It still reads well at 150% browser zoom.</li>
      <li>Tab reaches every row, and the focus ring is visible in both themes.</li>
    </ol>
  </section>
</main>
<script>
(() => {
  const root = document.documentElement;
  const accent = document.getElementById("accent");
  accent.addEventListener("change", () => { root.dataset.accent = accent.value; });
  const motion = document.getElementById("motion");
  motion.addEventListener("change", () => {
    if (motion.checked) root.dataset.motion = "reduce";
    else delete root.dataset.motion;
  });
  // One-shot animations restart when their element is replaced by a fresh copy.
  const replay = (scope) => scope.querySelectorAll(".status-glyph, .session-row[data-flash]").forEach((el) => el.replaceWith(el.cloneNode(true)));
  document.getElementById("replay").addEventListener("click", () => replay(document));
  document.querySelectorAll(".review__canvas button").forEach((b) => {
    if (b.textContent.trim() === "Replay") b.addEventListener("click", () => replay(b.closest(".theme-pair") ?? document));
  });
  // Looping animation pauses while the page is hidden, as in the app.
  const sync = () => root.toggleAttribute("data-page-hidden", document.visibilityState === "hidden");
  sync();
  document.addEventListener("visibilitychange", sync);
})();
</script>
`;

writeFileSync(out, html);
console.log(`wrote ${out} (${(html.length / 1024).toFixed(0)} KiB)`);
