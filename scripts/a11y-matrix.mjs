// Runs the review page through the accessibility matrix: Windows High Contrast, reduced motion, 150% zoom and four colour
// vision filters. For each it saves screenshots of the review sections into .a11y-matrix/ (git-ignored) and runs the in-page
// checks (scripts/a11y-checks.js). Exit code 1 when any check finds something.
//
// Usage: pnpm a11y:matrix <review.html | http://127.0.0.1:6011/review.html> [--only forced,zoom] [--sections launcher,command-palette,main-window]
// Needs Playwright:  pnpm add -D playwright && pnpm exec playwright install chromium   (see docs/a11y/a11y-matrix.md)
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const args = process.argv.slice(2);
const flag = (name) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : undefined;
};
const target = args.find((a) => !a.startsWith("--") && args[args.indexOf(a) - 1]?.startsWith("--") !== true);
if (!target) {
  console.error("usage: pnpm a11y:matrix <review.html | http://host/review.html> [--only forced,reduced,zoom,filters] [--sections id,id]");
  process.exit(2);
}
const url = /^https?:\/\//.test(target) ? target : pathToFileURL(resolve(target)).href;
const only = flag("only")?.split(",");
const sections = (flag("sections") ?? "needs-you-lane,main-window,launcher,command-palette,prompt-bar,manual-clis-and-session-numbers").split(",");

let chromium;
try {
  ({ chromium } = await import("playwright"));
} catch {
  console.error("Playwright is not installed. Run:  pnpm add -D playwright && pnpm exec playwright install chromium\nOr run the same checks by hand with the Playwright MCP: docs/a11y/a11y-matrix.md");
  process.exit(2);
}

const checks = readFileSync(new URL("./a11y-checks.js", import.meta.url), "utf8");
const out = resolve(".a11y-matrix");
mkdirSync(out, { recursive: true });

// Colour vision simulations (Machado et al. 2009, severity 1.0) and achromatopsia, as an SVG filter on the page.
const MATRICES = {
  protanopia: "0.152286 1.052583 -0.204868 0 0  0.114503 0.786281 0.099216 0 0  -0.003882 -0.048116 1.051998 0 0  0 0 0 1 0",
  deuteranopia: "0.367322 0.860646 -0.227968 0 0  0.280085 0.672501 0.047413 0 0  -0.011820 0.042940 0.968881 0 0  0 0 0 1 0",
  tritanopia: "1.255528 -0.076749 -0.178779 0 0  -0.078411 0.930809 0.147602 0 0  0.004733 0.691367 0.303900 0 0  0 0 0 1 0",
  achromatopsia: "0.2126 0.7152 0.0722 0 0  0.2126 0.7152 0.0722 0 0  0.2126 0.7152 0.0722 0 0  0 0 0 1 0",
};
const filterSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" style="position:absolute">${Object.entries(MATRICES)
  .map(([n, m]) => `<filter id="cvd-${n}" color-interpolation-filters="linearRGB"><feColorMatrix type="matrix" values="${m}"/></filter>`)
  .join("")}</svg>`;

const SCENARIOS = [
  { id: "forced", name: "forced-colors", context: { forcedColors: "active", viewport: { width: 1280, height: 900 } } },
  { id: "reduced", name: "reduced-motion", context: { reducedMotion: "reduce", viewport: { width: 1280, height: 900 } } },
  // 150% browser zoom of a 1280 px window is a 853 px layout viewport at device scale 1.5.
  { id: "zoom", name: "zoom-150", context: { viewport: { width: 853, height: 600 }, deviceScaleFactor: 1.5 } },
  ...Object.keys(MATRICES).map((n) => ({ id: "filters", name: `cvd-${n}`, context: { viewport: { width: 1280, height: 900 } }, filter: n })),
].filter((s) => !only || only.includes(s.id));

const browser = await chromium.launch();
const report = [];
let failed = 0;
for (const s of SCENARIOS) {
  const context = await browser.newContext(s.context);
  const page = await context.newPage();
  await page.goto(url);
  await page.waitForLoadState("load");
  await page.addScriptTag({ content: checks });
  if (s.filter) {
    await page.evaluate(([svg, f]) => {
      // The SVG is a constant defined above (no page or user content), parsed rather than assigned as markup.
      const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
      document.body.append(document.importNode(doc.documentElement, true));
      document.documentElement.style.filter = `url(#cvd-${f})`;
    }, [filterSvg, s.filter]);
  }
  const result = await page.evaluate(() => window.a11yChecks());
  for (const id of sections) {
    const el = page.locator(`section#${id}`);
    if ((await el.count()) === 0) continue;
    await el.first().screenshot({ path: `${out}/${s.name}--${id}.png` }).catch((e) => console.warn(`  screenshot ${id}: ${e.message.split("\n")[0]}`));
  }
  const n = result.findings.length;
  failed += n;
  report.push({ scenario: s.name, ...result });
  console.log(`${n === 0 ? "ok  " : "FAIL"} ${s.name}: ${n} finding(s)  ${JSON.stringify(result.counts)}`);
  const byRule = {};
  for (const f of result.findings) (byRule[f.rule] ??= []).push(f);
  for (const [rule, list] of Object.entries(byRule)) {
    console.log(`     ${rule} x${list.length}`);
    for (const f of list.slice(0, 4)) console.log(`       ${f.where}: ${f.detail}`);
  }
  await context.close();
}
await browser.close();
writeFileSync(`${out}/report.json`, JSON.stringify(report, null, 2));
console.log(`report: ${out}/report.json`);
process.exit(failed > 0 ? 1 : 0);
