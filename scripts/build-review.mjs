// Builds one self-contained design-review page from the batch's Storybook stories.
// Usage: node scripts/build-review.mjs <out.html> [1|2a|2b|2c]   (the page shows every batch up to and including the last)
// The stories are server-rendered with Vite's SSR loader; the CSS is the shipped token and component CSS, inlined.
import { readFileSync, writeFileSync } from "node:fs";
import { createServer } from "vite";

const out = process.argv[2];
if (!out) {
  console.error("usage: node scripts/build-review.mjs <out.html>");
  process.exit(1);
}

const batch = process.argv[3] ?? "2a";

const CSS = [
  "src/styles/tokens.css",
  "src/styles/accents.css",
  "src/styles/scrollbars.css",
  "src/styles/base.css",
  "src/styles/motion.css",
  "src/design/ThemePair.css",
  "src/design/specimen.css",
  "src/components/AgentMark/agent-colors.css",
  "src/components/AgentMark/AgentMark.css",
  "src/components/StatusGlyph/StatusGlyph.css",
  "src/components/ContextRing/ContextRing.css",
  "node_modules/@xterm/xterm/css/xterm.css",
  "src/features/terminal/ScriptedTerminal.css",
  "src/components/Chip/Chip.css",
  "src/components/Logo/Logo.css",
  "src/features/sidebar/SessionRow.css",
  "src/features/approval/ApproveCard.css",
  "src/features/lane/Lane.css",
  "src/features/window/Sidebar.css",
  "src/features/window/SessionHeader.css",
  "src/features/window/Rail.css",
  "src/features/window/MainWindow.css",
  "src/features/window/SplitView.css",
  "src/features/window/LayoutMenu.css",
];
const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"];

const server = await createServer({ appType: "custom", server: { middlewareMode: true }, logLevel: "error" });
let sections;
let terminalData;
try {
  const { renderReview } = await server.ssrLoadModule("/src/design/review/renderReview.tsx");
  sections = renderReview(batch);
  // The real xterm in the page needs the palettes and the canned session, read from the same sources as the app.
  const t = await server.ssrLoadModule("/src/features/terminal/palettes.ts");
  const sc = await server.ssrLoadModule("/src/features/terminal/scriptedSession.ts");
  terminalData = { palettes: t.PALETTES, minContrast: t.MIN_CONTRAST, session: sc.SCRIPTED_SESSION, size: sc.SCRIPTED_SIZE };
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
.review__section > h2 {
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

const META = {
  "2a": {
    lede: "Sub-review A of 3 in batch 2: the approve card in every state. Batch 1 (tokens, glyphs, rows) is below it, signed off. Windows and launcher follow once this is approved.",
    nav: [["approve-card", "Approve card"], ["sidebar-session-rows", "Rows"], ["design-colours", "Colours"], ["sign-off", "Sign-off"]],
    checks: [
      "From a glance you can tell who is asking, what for and how long they have waited.",
      "The risky card is unmistakable without colour: the warning line, the Hold to allow label and the missing Always.",
      "The 40-line command fades after four lines and Show all reveals it; a long unbroken token wraps instead of overflowing.",
      "The receipt, answered in terminal and timed-out states read as finished, with no hint of an undo.",
      "In Storybook: Tab to the interactive card, press Y before 500 ms (nothing happens), then Y, N, A, T, Space; Enter never allows.",
      "It still reads well at 150% browser zoom, and the focus ring is visible in both themes.",
    ],
  },
};
META["2b"] = {
  lede: "Sub-review B of 3 in batch 2: the main window with a real terminal. Sub-review A (approve card) and batch 1 are signed off and sit below.",
  nav: [["needs-you-lane", "Lane"], ["main-window", "Window"], ["approve-card", "Approve card"], ["sidebar-session-rows", "Rows"], ["design-colours", "Colours"], ["sign-off", "Sign-off"]],
  checks: [
    "In 5 seconds you can name which two sessions need you and why (the 5-second test).",
    "The lane header is always there; the oldest request is the expanded card; Twelve waiting scrolls and says +11 more.",
    "Split view: the active pane is obvious, the other header recedes, terminal text is not dimmed.",
    "Focus mode, the rail, the 940 px auto-collapse and the 720 px window all hold together.",
    "Pin the terminal light while the app is dark (and the reverse) with the Terminal theme control: the palette changes at once, nothing re-flows, dim text is lifted by Contrast protection.",
    "It still reads well at 150% browser zoom, and the focus ring is visible in both themes.",
  ],
};
const meta = META[batch];
if (!meta) {
  console.error(`no page text for batch "${batch}" in scripts/build-review.mjs (known: ${Object.keys(META).join(", ")})`);
  process.exit(1);
}

const withTerminal = ["2b", "2c"].includes(batch);
const safeInline = (text) => text.replace(/<\/script/gi, "<\\/script");
const terminalScripts = withTerminal
  ? `<script>${safeInline(readFileSync("node_modules/@xterm/xterm/lib/xterm.js", "utf8"))}</script>
<script>${safeInline(readFileSync("node_modules/@xterm/addon-fit/lib/addon-fit.js", "utf8"))}</script>
<script type="application/json" id="terminal-data">${JSON.stringify(terminalData).replace(/<\//g, "<\\/")}</script>
<script>${safeInline(readFileSync("scripts/review-terminal.js", "utf8"))}</script>`
  : "";

const options = ACCENTS.map((a) => `<option value="${a}">${a[0].toUpperCase()}${a.slice(1)}</option>`).join("");

const html = `<title>Marshell design review</title>
<style>
${css}
${chrome}
</style>
<main class="review">
  <header>
    <h1>Marshell design review</h1>
    <p class="review__lede">${meta.lede}</p>
  </header>
  <div class="review__bar">
    <label for="accent">Accent <select id="accent">${options}</select></label>
    <label for="motion"><input type="checkbox" id="motion"> Reduced motion</label>
    <button type="button" id="replay">Replay animations</button>
    ${withTerminal ? `<label for="terminal-theme">Terminal theme <select id="terminal-theme"><option value="follow-app">Follow app</option><option value="dark">Always dark</option><option value="light">Always light</option></select></label><label for="terminal-contrast"><input type="checkbox" id="terminal-contrast" checked> Contrast protection</label>` : ""}
    <nav aria-label="Sections">
      ${meta.nav.map(([id, text]) => `<a href="#${id}">${text}</a>`).join("\n      ")}
    </nav>
  </div>
  ${sections}
  <section class="review__section" id="sign-off">
    <h2>Sign-off checklist</h2>
    <ol class="review__check">
      ${meta.checks.map((c) => `<li>${c}</li>`).join("\n      ")}
    </ol>
  </section>
</main>
${terminalScripts}
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
