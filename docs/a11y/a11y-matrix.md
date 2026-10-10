# The accessibility matrix

`pnpm a11y:matrix <review.html | http://127.0.0.1:6011/review.html>` opens the built review page under each condition below, saves screenshots to `.a11y-matrix/` (git-ignored) and runs the in-page checks in `scripts/a11y-checks.js`. It exits with 1 when any check finds something.

| Condition | How it is set | What is checked |
| --- | --- | --- |
| Windows High Contrast | `forcedColors: "active"` | Every selected item (`aria-selected`, `aria-current`, `data-selected` = true) has an outline or a fill that differs from the page |
| Reduced motion | `reducedMotion: "reduce"` | No animation is still looping |
| 150% zoom | 853 px layout width at device scale 1.5 (a 1280 px window at 150%) | No box clips text without an ellipsis |
| Protanopia, deuteranopia, tritanopia, achromatopsia | An SVG colour-matrix filter on the page | Screenshots only; read them: every status must still be told apart by shape and words (a table-driven unit test, `src/components/a11y/rules.test.tsx`, enforces the shape difference) |
| Every run | | Keyboard focus shows on every kind of control (an outline, or a visible change on the control or its bar); at most one live region per window |

## Running it

The script needs Playwright, which the repo does not install by default:

```
pnpm add -D playwright
pnpm exec playwright install chromium
pnpm review:build .a11y-matrix/review.html 2c
pnpm a11y:matrix .a11y-matrix/review.html
```

Options: `--only forced,reduced,zoom,filters` and `--sections launcher,main-window`.

## Without installing Playwright (the Playwright MCP, as used during phase 0)

1. Copy `scripts/a11y-checks.js` next to the served review page.
2. `browser_emulate_media` with `forcedColors: "active"` (or `reducedMotion: "reduce"`), `browser_resize` to 853×600 for 150% zoom, then `browser_navigate` to the page.
3. `browser_evaluate`: fetch the script, `eval` it, call `window.a11yChecks()`. The result lists `findings` with a rule, a location and a detail.

## Known findings (phase 0, batch 2)

- **F-1, focus on the full prompt line.** The single-terminal prompt line (`Composer variant="full"`) defaults to `focused`, so its vendor-coloured top edge is on before the field has focus, and focusing the field changes nothing visible. A keyboard user cannot see that the field has focus. The fix is a design choice (show the edge only on `:focus-within` for the full variant too, or add a focus-visible outline on the field), not a one-line change.

Everything else the matrix checks was clean when it was first run on the 2c review page: forced colours, reduced motion and 150% zoom had no other findings.
