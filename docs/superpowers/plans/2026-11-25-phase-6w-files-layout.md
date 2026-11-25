# Slice 6w: the Files panel's layout and the pane's rounded tabs. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Files panel reads as two rows (the tab strip, then a path bar across the whole panel) over the file and the tree, in every panel width and height. Under 480 px the tree takes the file's place. A short empty state gives up parts and never scrolls. The pane's tabs are rounded and told apart by fill.

**Architecture:**
- **Body.** `FilesBody` becomes a two-row grid: the path bar, then a main row that holds the tree, the divider and the file. The filter box moves nowhere in the DOM; it is already the tree column's first child, and it gets an inset and a search mark.
- **Narrow.** Under 480 px the main row is one grid cell. The tree is laid over the file in that cell and covers it whole; the file stays mounted and is `inert`.
- **File tools.** The path bar gets an empty slot element. `PreviewPane` reads it from a context and, in the Files panel, portals its tools there instead of drawing its own header. Back and Forward are not drawn in Files.
- **Empty state.** `EmptyPanel` measures: it tries level 0 (everything), 1 (no mark, no help line), 2 (no list) and keeps the first that fits. The level is a `data-level` attribute written straight to the DOM; CSS does the hiding.
- **Tabs.** CSS only, in `SidePane.css`: a tab is a 24 px rounded shape inside the 28 px strip, and the selected one is filled.

**Tech Stack:** React 19, TypeScript, CSS, Vitest. No new dependency. No Rust, no protocol type, nothing under `src-tauri/`.

**Spec:** `docs/PLAN.md` §9, "Slice 6w decisions (2026-11-25)", decisions 1 to 11. The pictures of what was wrong are `C:\dev\Airport\.playwright-mcp\lc-6v2-2-files-*.png` (main checkout, not in git).

## Global Constraints

- Frontend only. Nothing under `crates/`, `src-tauri/`, `src/generated/` or `.github/` changes.
- No new dependency.
- Read the `good-css` skill before writing declarations. Logical properties only (`inline`, `block`); no `left`, `right`, `top`, `bottom`. Colours from tokens (`src/styles/tokens.css`); no hex. Every `:hover` inside `@media (hover: hover) and (pointer: fine)`. Never `outline: none`. A `forced-colors` block names system colours only.
- No boxed outlines: shapes are told apart by fill (`noBoxedOutlines.test.ts`).
- The window shows no scrollbar.
- Every change works with the keyboard alone and in forced colours.
- Vitest runs in node with no DOM. Components are tested through `renderToStaticMarkup`, logic through pure functions, CSS by reading the file from disk (as `FilesBody.css.test.ts` does).
- Copy is fixed by this plan. This slice adds no new user-facing sentence; every label and announcement keeps its words.
- Tokens used below: `--space-1` 4 px, `--space-2` 8 px, `--space-3` 12 px, `--radius` 6 px, `--hairline`, `--fill`, `--fill-strong`, `--hover`, `--text-1/2/3`, `--bg-base`.
- Each task: write the test first and watch it fail. One file: `pnpm test <path>`. Before a task's commit: `pnpm test src/features/files src/features/pane src/features/preview` and `pnpm build`.
- Commit messages: `type(scope): what changed, as a sentence.`
- Workers: stay inside the task's files; never `git checkout`, `git restore` or `git stash` anything; scratch files go under the system temp folder, in a folder of the task's own.
- Verified before writing (2026-11-25, at `b91e4f33`):
  - `FilesBody` puts the path bar inside the file column (`FilesBody.tsx:86–91`) and places the narrow tree with `position: absolute` (`FilesBody.css:136–148`).
  - `PreviewPane`'s header is `Preview.tsx:579–616`. `pathShown` is true only for the Files panel (`filesActions.ts:86`). `goBack` and `goForward` (`Preview.tsx:353–362`) also run on Alt+Left and Alt+Right (`Preview.tsx:553–558`).
  - `focusPane` makes its first attempt after a frame (`focusPane.ts:68–94`), so React has committed by then.
  - `EmptyPanel` is shared by Browser, Files and Changes (`pane/EmptyBrowser.tsx`, `pane/EmptyFiles.tsx`, `changes/`). Its short rule today is `@container (height < 280px)` (`EmptyPanel.css`), pinned by `EmptyPanel.css.test.ts:60`.
  - The strip is 28 px (`SidePane.css:8`), its buttons 24 px (`SidePane.css:172–177`).
  - `:not([inert] *)` and `:has()` work in WebView2, WKWebView 15.4+ and WebKitGTK 2.42+.
- Items marked "not verified" are for the worker to check first, and to report if the code says otherwise.

## Review Focus

1. **A file under the narrow tree can't be reached.** With the tree up under 480 px, Tab from the tree must never land in the editor behind it, and `focusPane("files")` must put the keyboard on a tree row, not on the hidden editor. (Task 2 `skips what is inert`; live-check item 4.)
2. **The tools belong to the tab on screen only.** Four kept tabs are mounted at once; only the selected one may portal into the slot, and switching tabs must swap the tools, not add to them. A tab with no tools (plain text) leaves the slot empty and takes no room. (Task 3 `headerPlace`; live-check item 6.)
3. **The preview still knows the keyboard is inside it** when focus is on a tool that now lives in the path bar: the "Updated" note and the deleted notice must stay polite, and R, Esc, Alt+Left and Alt+Right must still work from a tool button. (Task 3 `insidePane`; live-check item 6.)
4. **The fit must not loop or flicker.** Writing `data-level` must not change the container's own size, a panel dragged taller must get its mark and list back, and a list that grows while it is hidden must be judged again. (Task 4 `firstFit`; live-check item 8.)
5. **Width extremes.** A 240 px file column with a Markdown file's tools must keep the tree button in view (the path scrolls, the buttons never shrink). At 320 px of pane the two empty-state buttons may wrap. (Task 1 and Task 3 CSS tests; live-check items 2 and 8.)

---

### Task 1: The path bar across the panel, and the filter set into the tree's column

**Files:**
- Modify: `src/features/files/FilesBody.tsx`, `src/features/files/FilesBody.css`, `src/features/files/TreeFilterBox.tsx`, `src/features/files/FileTree.css`
- Test: `src/features/files/FilesBody.test.tsx`, `src/features/files/FilesBody.css.test.ts`, `src/features/files/TreeFilterBox.test.tsx`, `src/features/files/FileTree.css.test.ts`

**Interfaces:**
- Produces: `FilesBody`'s markup is `div.files-body > (the path bar, div.files-body__main)`. `.files-body` carries `data-tree=""` while the tree is drawn. `FilesBodyProps` is unchanged. Task 2 changes the narrow branch of the same component.
- Produces: `TreeFilter` draws `div.file-tree-filter-box > (input.file-tree-filter, svg.file-tree-filter__mark)`. Its props are unchanged, and the input keeps its class, its `aria-label="Filter files"` and its placeholder.

- [ ] **Step 1: Write the failing tests**

`FilesBody.test.tsx` (replace `the DOM order follows the reading order` and `a hidden tree leaves the file and the bar alone`; keep the divider test):

```tsx
it("the path bar is the first row and the main row holds the tree, the divider and the file", () => {
  const html = body({ panelWidth: 800, position: "left" }); // the file's helper, with pathBar=<div class="path-bar"/>
  expect(html.indexOf('class="path-bar"')).toBeLessThan(html.indexOf("files-body__main"));
  const main = html.slice(html.indexOf("files-body__main"));
  expect(main.indexOf("files-body__tree")).toBeLessThan(main.indexOf("files-body__divider"));
  expect(main.indexOf("files-body__divider")).toBeLessThan(main.indexOf("files-body__file"));
  expect(main).not.toContain("path-bar");
});
it("a tree on the end edge comes after the file", () => {
  const main = body({ panelWidth: 800, position: "right" }).split("files-body__main")[1];
  expect(main.indexOf("files-body__file")).toBeLessThan(main.indexOf("files-body__tree"));
});
it("says whether the tree is drawn", () => {
  expect(body({ panelWidth: 800, shown: true })).toContain('data-tree=""');
  expect(body({ panelWidth: 800, shown: false })).not.toContain("data-tree");
  expect(body({ panelWidth: 800, shown: false })).toContain('class="path-bar"');
});
```

`FilesBody.css.test.ts` (add; `rule(sel)` is the slice from `sel + " {"` to the next `}`):

```ts
it("is two rows: the path bar, then the tree and the file", () => {
  expect(rule(".files-body")).toContain("grid-template-rows: auto minmax(0, 1fr)");
  expect(rule(".files-body__main")).toContain("display: flex");
});
it("draws one line under the path bar, across the panel", () => {
  expect(rule(".path-bar")).toContain("border-block-end: 1px solid var(--hairline)");
});
it("never shrinks the path bar's buttons: the path gives way and scrolls", () => {
  expect(rule(".path-bar__button")).toContain("flex: none");
  expect(rule(".path-bar__path")).toContain("min-inline-size: 0");
});
it("leaves a gutter between the code and a tree that follows it", () => {
  expect(css).toContain('.files-body[data-edge="end"][data-tree]:not([data-narrow]) .files-body__content .cm-scroller');
  expect(css).toMatch(/\.cm-scroller \{\s*margin-inline-end: var\(--space-2\);/);
});
```

`TreeFilterBox.test.tsx` (add):

```tsx
it("has a search mark the reader never hears, drawn after the box so it paints over it", () => {
  const html = renderToStaticMarkup(<TreeFilter value="" onChange={() => {}} onEscape={() => {}} onDown={() => {}} />);
  expect(html).toMatch(/<div class="file-tree-filter-box"><input class="file-tree-filter"[^>]*aria-label="Filter files"[^>]*><svg class="file-tree-filter__mark"[^>]*aria-hidden="true"/);
});
```

`FileTree.css.test.ts` (add):

```ts
it("sets the filter in from the column's edges and stacks its mark in the same cell", () => {
  expect(rule(".file-tree-filter-box")).toContain("display: grid");
  expect(rule(".file-tree-filter-box")).toContain("padding: var(--space-2)");
  expect(rule(".file-tree-filter-box > *")).toContain("grid-area: 1 / 1");
  expect(rule(".file-tree-filter")).toContain("block-size: 28px");
  expect(rule(".file-tree-filter__mark")).toContain("pointer-events: none");
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm test src/features/files/FilesBody.test.tsx src/features/files/FilesBody.css.test.ts src/features/files/TreeFilterBox.test.tsx src/features/files/FileTree.css.test.ts`
Expected: the new tests FAIL (no `files-body__main`, no `file-tree-filter-box`).

- [ ] **Step 3: `FilesBody.tsx`**

- The file column is `<div key="file" className="files-body__file"><div className="files-body__content">{file}</div></div>`; `pathBar` leaves it.
- The return is `<div className="files-body" data-side data-edge data-narrow data-tree={showTree ? "" : undefined} data-rtl>{pathBar}<div className="files-body__main">{parts}</div></div>`.
- `parts` and everything else stay as they are in this task (the narrow branch is Task 2's). Update the component's doc comment: the path bar runs across the panel.

- [ ] **Step 4: `FilesBody.css`**

- `.files-body`: `display: grid; grid-template-rows: auto minmax(0, 1fr);` in place of `display: flex`. Keep the rest.
- New `.files-body__main { display: flex; min-inline-size: 0; min-block-size: 0; }`.
- `.path-bar`: add `border-block-end: 1px solid var(--hairline);`.
- New, after the divider's rules:

```css
/* A gutter between the code and a tree that follows it, so a long line stops short of the divider. */
.files-body[data-edge="end"][data-tree]:not([data-narrow]) .files-body__content .cm-scroller {
  margin-inline-end: var(--space-2);
}
```

Not verified: that a margin on `.cm-scroller` leaves the editor's status line (`src/features/editor/EditorFooter.tsx`) at the column's full width. If the status line moves with it, report it; the live check judges item 2.

- [ ] **Step 5: `TreeFilterBox.tsx` and `FileTree.css`**

- Wrap the input: `<div className="file-tree-filter-box">{input}<svg className="file-tree-filter__mark" width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><circle cx="6" cy="6" r="3.75" /><path d="m8.75 8.75 3.25 3.25" /></svg></div>` (the same drawing as `SearchIcon` in `pane/EmptyPanel.tsx`).
- CSS:

```css
/* The filter box, set in from the column's edges; its mark is stacked in the same cell, over the box's start. */
.file-tree-filter-box {
  display: grid;
  padding: var(--space-2);
}
.file-tree-filter-box > * {
  grid-area: 1 / 1;
}
.file-tree-filter__mark {
  place-self: center start;
  margin-inline-start: var(--space-2);
  color: var(--text-3);
  pointer-events: none;
}
```

- `.file-tree-filter`: `block-size: 28px; padding-block: 0; padding-inline: calc(var(--space-2) * 2 + 14px) var(--space-2);` in place of its `padding`. In the forced-colors block add `.file-tree-filter__mark { color: CanvasText; }`.
- The note under the box (`FilesPanel.tsx`, the `<p className="file-tree__none">`) keeps its place and its inline padding.

- [ ] **Step 6: Run the tests, then the build**

Run: `pnpm test src/features/files src/features/pane src/features/preview` then `pnpm build`
Expected: all pass. A test elsewhere that pinned the old order (search for `files-body__file` in `src/**/*.test.*`) is updated to the new markup, not deleted.

- [ ] **Step 7: Commit**

```bash
git add src/features/files
git commit -m "fix(files): the path bar runs across the Files panel, and the filter box is set into the tree's column."
```

---

### Task 2: Under 480 px the tree takes the file's place

**Files:**
- Modify: `src/features/files/FilesBody.tsx`, `src/features/files/FilesBody.css`, `src/features/files/treeLayout.ts`, `src/features/files/filesActions.ts`, `src/features/files/FilesPanel.tsx`, `src/features/files/PathBar.tsx`, `src/features/pane/focusPane.ts`
- Test: `src/features/files/FilesBody.test.tsx`, `src/features/files/FilesBody.css.test.ts`, `src/features/files/treeLayout.test.ts`, `src/features/files/filesActions.test.ts`, `src/features/files/PathBar.test.tsx`, `src/features/pane/focusPane.test.ts`

**Interfaces:**
- Consumes: Task 1's markup.
- Produces, in `filesActions.ts`:
  - `afterOpen(how: "click" | "enter" | "double" | "space", o: { narrow: boolean; panel: PanelId }): { close: boolean; focus: PanelId | "tree-button" | null }`
  - `treeBack(before: string | null, now: string | null): boolean` (true when the last file has just closed)
- Removes: `narrowAfterOpen` from `treeLayout.ts`, and its test.
- Produces: the path bar's tree button carries `data-tree-button=""`.

- [ ] **Step 1: Write the failing tests**

`filesActions.test.ts`, in `describe("afterOpen")`, replace the narrow cases:

```ts
it("in the narrow layout every open shows the file; Space leaves the keyboard on the tree button, out of the editor", () => {
  expect(afterOpen("click", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "files" });
  expect(afterOpen("enter", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "files" });
  expect(afterOpen("double", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "files" });
  expect(afterOpen("space", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "tree-button" });
});
it("in the wide layout Space still leaves the tree and the keyboard as they are", () => {
  expect(afterOpen("space", { narrow: false, panel: "files" })).toEqual({ close: false, focus: null });
});
describe("treeBack", () => {
  it("is true only when the last file closes", () => {
    expect(treeBack("C:\\p\\a.ts", null)).toBe(true);
    expect(treeBack(null, null)).toBe(false);
    expect(treeBack(null, "C:\\p\\a.ts")).toBe(false);
    expect(treeBack("C:\\p\\a.ts", "C:\\p\\b.ts")).toBe(false);
  });
});
```

Keep the two `panel: "browser"` cases as they are. Delete `only Space leaves the tree over the file` from `treeLayout.test.ts` and `narrowAfterOpen` from its import.

`FilesBody.test.tsx` (replace `under 480 px the tree is drawn over the file` and `the narrow tree is capped 40 px short of the panel`):

```tsx
it("under 480 px the tree takes the file's place: full width, the file inert under it, no divider", () => {
  const html = body({ panelWidth: 400, overlayOpen: true });
  expect(html).toContain('data-narrow=""');
  expect(html).not.toContain("files-body__divider");
  expect(html).toMatch(/class="files-body__file" inert=""/);
  expect(html).toMatch(/class="files-body__tree"(?! style)/);
  expect(html.indexOf("files-body__file")).toBeLessThan(html.indexOf("files-body__tree"));
});
it("under 480 px with the tree closed the file is alone and can be used", () => {
  const html = body({ panelWidth: 400, overlayOpen: false });
  expect(html).not.toContain("files-body__tree");
  expect(html).not.toContain("inert");
});
it("a wide panel never makes the file inert", () => {
  expect(body({ panelWidth: 800 })).not.toContain("inert");
});
```

`FilesBody.css.test.ts` (add):

```ts
it("lays the narrow tree over the file in one cell, with no absolute placement", () => {
  expect(rule(".files-body[data-narrow] .files-body__main")).toContain("display: grid");
  expect(rule(".files-body[data-narrow] .files-body__main > *")).toContain("grid-area: 1 / 1");
  expect(css).not.toContain("position: absolute");
  expect(css).not.toContain("inset-block: 0;\n  z-index: 2");
});
```

(The divider's `::before` keeps `position: absolute`; scope the `not.toContain` to the text before `/* The divider` so it holds.)

`PathBar.test.tsx` (add to `has the two buttons…`): `expect(html).toMatch(/<button[^>]*aria-label="Show file tree"[^>]*data-tree-button=""/)` (attribute order as React writes it; match either order).

`focusPane.test.ts` (add):

```ts
it("skips what is inert: an editor or a body under the narrow tree never takes the keyboard", () => {
  const seen: string[] = [];
  paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "files");
  expect(seen.find((s) => s.includes("cm-content"))).toContain(":not([inert] *)");
  expect(seen.find((s) => s.includes("preview__body"))).toContain(":not([inert] *)");
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm test src/features/files src/features/pane/focusPane.test.ts`
Expected: the new tests FAIL.

- [ ] **Step 3: `treeLayout.ts` and `filesActions.ts`**

Remove `narrowAfterOpen`. `afterOpen` for the Files panel: wide is `{ close: false, focus: keep ? "files" : null }`; narrow is `{ close: true, focus: how === "space" ? "tree-button" : "files" }`. Add `treeBack` as `before !== null && now === null`. Update both doc comments (decisions 1 and 7).

- [ ] **Step 4: `FilesBody.tsx` and `FilesBody.css`**

- `const covered = narrow && showTree;` The file column gets `inert={covered}`. In the narrow layout the tree column has no `style`; `px` is `treeWidth(width, panelWidth)` in the wide layout only (delete the narrow `Math.min(…, panelWidth - 40)` branch).
- `parts` is unchanged: narrow is `[fileColumn, treeColumn]`, so the tree is later in the DOM and paints over the file.
- CSS: delete the three `.files-body[data-narrow]…__tree` rules (`position: absolute`, the two `data-edge` borders) and `position: relative` on `.files-body`. Add:

```css
/* Under 480 px the tree takes the file's place: both sit in one cell and the tree, later in the DOM, covers the file whole. */
.files-body[data-narrow] .files-body__main {
  display: grid;
}
.files-body[data-narrow] .files-body__main > * {
  grid-area: 1 / 1;
}
```

The tree keeps its opaque `background: var(--bg-base)` and, in forced colours, `Canvas` (the rule is there already).

- Update the component's doc comment and the `onTreeKey` comment: Esc in the narrow tree still closes it first.

- [ ] **Step 5: `PathBar.tsx`, `FilesPanel.tsx`, `focusPane.ts`**

- `PathBar`: `data-tree-button=""` on the "Show file tree" button.
- `FilesPanel.openAt`: after `setOverlay(false)`, `if (after.focus === "tree-button") box.current?.querySelector<HTMLElement>("[data-tree-button]")?.focus(); else if (after.focus !== null) focusPane(after.focus);`. Update its comment.
- `FilesPanel`: a ref holds the last `activePath`; an effect on `[activePath]` calls `setOverlay(true)` when `treeBack(last, activePath)`, then stores the new value.
- `focusPane.ts`: the `editor` and `body` selectors end with `:not([inert] *)`. Say why in their comments. The `slot` selector is left alone (no browser tab sits in the Files body).

- [ ] **Step 6: Run the tests, then the build**

Run: `pnpm test src/features/files src/features/pane src/features/preview src/features/sessions` then `pnpm build`
Expected: all pass. `FilesPanel.test.tsx` and `LiveApp.test.tsx` cases that pinned the old narrow behaviour are updated to decision 1 and decision 7, not deleted.

- [ ] **Step 7: Commit**

```bash
git add src/features/files src/features/pane
git commit -m "fix(files): under 480 px the tree takes the file's place, and Space there shows the file."
```

---

### Task 3: A file's tools sit in the path bar, and Files has no Back and Forward

**Files:**
- Create: `src/features/preview/headerSlot.ts`, `src/features/preview/headerSlot.test.ts`
- Modify: `src/features/preview/Preview.tsx`, `src/features/files/PathBar.tsx`, `src/features/files/FilesPanel.tsx`, `src/features/files/FilesBody.css`
- Test: `src/features/preview/Preview.test.tsx`, `src/features/files/PathBar.test.tsx`, `src/features/files/FilesBody.css.test.ts`

**Interfaces:**
- Produces, in `preview/headerSlot.ts`:
  - `export const HeaderSlot = createContext<HTMLElement | null>(null);` (the element a Files tab's tools are drawn into)
  - `export function headerPlace(o: { pathShown: boolean; active: boolean; slot: boolean }): "header" | "slot" | "none"`: `"header"` when `!pathShown`; else `"slot"` when `active && slot`; else `"none"`.
  - `export function insidePane(pane: Pick<Node, "contains">, slot: Pick<Node, "contains"> | null, to: Node | null): boolean`: whether `to` is in the pane or in the slot.
- Produces: `PathBarProps` gains `toolsRef?: (el: HTMLElement | null) => void`; the bar draws `<div className="path-bar__tools" ref={toolsRef} />` between the `<nav>` and "Collapse all folders".

- [ ] **Step 1: Write the failing tests**

`headerSlot.test.ts`:

```ts
describe("headerPlace", () => {
  it("draws the pane's own header outside the Files panel", () => {
    expect(headerPlace({ pathShown: false, active: true, slot: false })).toBe("header");
    expect(headerPlace({ pathShown: false, active: false, slot: true })).toBe("header");
  });
  it("in Files only the tab on screen draws its tools, and only into the path bar", () => {
    expect(headerPlace({ pathShown: true, active: true, slot: true })).toBe("slot");
    expect(headerPlace({ pathShown: true, active: false, slot: true })).toBe("none");
    expect(headerPlace({ pathShown: true, active: true, slot: false })).toBe("none");
  });
});
describe("insidePane", () => {
  const node = (inside: unknown[]) => ({ contains: (n: unknown) => inside.includes(n) }) as Pick<Node, "contains">;
  const a = {} as Node, b = {} as Node, c = {} as Node;
  it("counts focus on a tool in the path bar as inside the preview", () => {
    expect(insidePane(node([a]), node([b]), a)).toBe(true);
    expect(insidePane(node([a]), node([b]), b)).toBe(true);
    expect(insidePane(node([a]), node([b]), c)).toBe(false);
    expect(insidePane(node([a]), null, b)).toBe(false);
    expect(insidePane(node([a]), node([b]), null)).toBe(false);
  });
});
```

`Preview.test.tsx` (add; use the file's existing way of rendering `PreviewPane`):

```tsx
it("in the Files panel it draws no header of its own and no Back or Forward", () => {
  const html = pane({ pathShown: true }); // a Markdown entry with text loaded
  expect(html).not.toContain("preview__header");
  expect(html).not.toContain('aria-label="Back"');
  expect(html).not.toContain('aria-label="Forward"');
});
```

The existing header tests (11 lines name Back or Forward) render without `pathShown` and stay as they are.

`PathBar.test.tsx` (add):

```tsx
it("has a place for the file's tools between the path and its own buttons", () => {
  const html = bar();
  expect(html.indexOf("</nav>")).toBeLessThan(html.indexOf("path-bar__tools"));
  expect(html.indexOf("path-bar__tools")).toBeLessThan(html.indexOf('aria-label="Collapse all folders"'));
});
```

`FilesBody.css.test.ts` (add):

```ts
it("an empty tools place takes no room, and a full one never shrinks", () => {
  expect(rule(".path-bar__tools")).toContain("flex: none");
  expect(rule(".path-bar__tools:empty")).toContain("display: none");
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm test src/features/preview src/features/files/PathBar.test.tsx src/features/files/FilesBody.css.test.ts`
Expected: FAIL (`headerSlot` is missing; the header is still drawn).

- [ ] **Step 3: `headerSlot.ts`**

The three exports of the Interfaces block, each with a one-line doc comment.

- [ ] **Step 4: `Preview.tsx`**

- `const slot = useContext(HeaderSlot); const place = headerPlace({ pathShown, active, slot: slot !== null });`
- Split the header's children into `tools` (the size `preview__meta`, `preview__updated`, the `preview__views` group, `ViewportChip`, "Select region") and the rest (Back, Forward, the name and folder, `preview__spacer`).
- `place === "header"`: the header exactly as today (Back, Forward, name, folder, the tools with the spacer where it is now). `place === "slot"`: `createPortal(tools, slot)` and no `<header>`. `place === "none"`: neither.
- `onBlur` uses `insidePane(e.currentTarget, slot, e.relatedTarget as Node | null)`.
- Alt+Left and Alt+Right stay as they are (decision 11). Not verified: that `active` is false for the kept tabs that are not selected; check `KeptTabs.tsx`'s `render(tab, active)` and report if not.
- A React portal's events bubble through the React tree, so the section's `onKeyDown` and `onFocus` still hear the tools. Say so in a comment above the portal.

- [ ] **Step 5: `PathBar.tsx`, `FilesPanel.tsx`, `FilesBody.css`**

- `PathBar`: the `path-bar__tools` div of the Interfaces block.
- `FilesPanel`: `const [tools, setTools] = useState<HTMLElement | null>(null);` passes `toolsRef={setTools}` to `PathBar` and wraps its returned tree in `<HeaderSlot value={tools}>`. (`p.file` is created by `LiveApp` and rendered inside `FilesPanel`, so the context reaches it.)
- CSS:

```css
/* The open file's tools (Rendered and Source, Select region, an image's size), drawn here by its tab. Empty for a plain text file. */
.path-bar__tools {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--text-12);
}
.path-bar__tools:empty {
  display: none;
}
```

Not verified: that `.preview__views` and `.preview__button` (from `Preview.css`) need nothing more inside the bar. The live check judges the look (item 6); fix what it shows in this file.

- [ ] **Step 6: Run the tests, then the build**

Run: `pnpm test src/features/files src/features/pane src/features/preview src/features/sessions` then `pnpm build`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/features/preview src/features/files
git commit -m "fix(files): a file's tools sit in the path bar, and Files draws no Back and Forward."
```

---

### Task 4: A short empty state gives up parts and never scrolls

**Files:**
- Create: `src/features/pane/emptyFit.ts`, `src/features/pane/emptyFit.test.ts`
- Modify: `src/features/pane/EmptyPanel.tsx`, `src/features/pane/EmptyPanel.css`
- Test: `src/features/pane/EmptyPanel.css.test.ts`, `src/features/pane/EmptyPanel.test.tsx`

**Interfaces:**
- Produces, in `pane/emptyFit.ts`:
  - `export type FitLevel = 0 | 1 | 2;` (0: everything; 1: no mark and no help line; 2: no list either)
  - `export function firstFit(room: number, heightAt: (level: FitLevel) => number): FitLevel`: the lowest level whose height is at most `room`, trying 0, 1, 2 in order and asking `heightAt` for no level after the one that fits; 2 when none fits.
  - `export function fitEmpty(panel: HTMLElement, column: HTMLElement): void`: sets `panel.dataset.level` to `firstFit(panel.clientHeight, (l) => { panel.dataset.level = String(l); return column.offsetHeight; })`.

- [ ] **Step 1: Write the failing tests**

`emptyFit.test.ts`:

```ts
describe("firstFit", () => {
  const heights = (h: [number, number, number]) => { const asked: number[] = []; return { at: (l: FitLevel) => (asked.push(l), h[l]), asked }; };
  it("keeps everything when it fits, and measures nothing more", () => {
    const m = heights([400, 300, 90]);
    expect(firstFit(400, m.at)).toBe(0);
    expect(m.asked).toEqual([0]);
  });
  it("gives up the mark and the help line first, then the whole list", () => {
    expect(firstFit(399, heights([400, 300, 90]).at)).toBe(1);
    expect(firstFit(299, heights([400, 300, 90]).at)).toBe(2);
  });
  it("stays at the last level when even that is too tall", () => {
    expect(firstFit(50, heights([400, 300, 90]).at)).toBe(2);
  });
});
describe("fitEmpty", () => {
  it("leaves the level it found on the panel", () => {
    const panel = { clientHeight: 250, dataset: {} as Record<string, string> };
    const column = { get offsetHeight() { return [400, 300, 90][Number(panel.dataset.level)]; } };
    fitEmpty(panel as unknown as HTMLElement, column as unknown as HTMLElement);
    expect(panel.dataset.level).toBe("2");
  });
});
```

`EmptyPanel.css.test.ts`: replace `drops the icon, never a button, in a short panel` with:

```ts
it("gives up the mark and the help line at level 1 and the list at level 2, never the heading or a button", () => {
  expect(css).toMatch(/\.empty-panel\[data-level="1"\] \.empty-panel__icon,\s*\.empty-panel\[data-level="2"\] \.empty-panel__icon,\s*\.empty-panel\[data-level="1"\] \.empty-panel__line,\s*\.empty-panel\[data-level="2"\] \.empty-panel__line \{\s*display: none;/);
  expect(css).toMatch(/\.empty-panel\[data-level="2"\] \.empty-panel__group \{\s*display: none;/);
  expect(css).not.toMatch(/data-level[^{]*__(title|actions|button)[^{]*\{\s*display: none/);
  expect(css).not.toContain("@container");
});
it("shows no scrollbar", () => {
  expect(rule(".empty-panel")).toContain("scrollbar-width: none");
});
```

`EmptyPanel.test.tsx` (add): the static markup has no `data-level` (the server render is level 0), and the column carries `class="empty-panel__column"` as before.

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm test src/features/pane/emptyFit.test.ts src/features/pane/EmptyPanel.css.test.ts`
Expected: FAIL.

- [ ] **Step 3: `emptyFit.ts`**

As the Interfaces block. `fitEmpty`'s comment says why it writes the attribute itself: three synchronous tries before paint, and no React state to loop on.

- [ ] **Step 4: `EmptyPanel.tsx`**

Refs on the panel and the column. A `useLayoutEffect` with no dependency list runs `fitEmpty` after every render (the list's length may have changed). A second `useLayoutEffect` (empty list) puts a `ResizeObserver` on the panel that runs `fitEmpty`, guarded by `typeof ResizeObserver === "undefined"`, and disconnects on unmount. `data-level` is never in the JSX, so React never resets it.

- [ ] **Step 5: `EmptyPanel.css`**

- Delete the `@container (height < 280px)` block and `container-type: size` on `.empty-panel`; update the file's top comment (a short panel gives up its mark and help line, then its list; it never scrolls).
- `.empty-panel`: add `scrollbar-width: none;` and a `.empty-panel::-webkit-scrollbar { display: none; }` rule. `overflow-y: auto` stays as the last resort for a panel too short for level 2.
- Add:

```css
/* What a short panel gives up, by the level EmptyPanel measured (emptyFit.ts): the mark and the help line, then the whole list. */
.empty-panel[data-level="1"] .empty-panel__icon,
.empty-panel[data-level="2"] .empty-panel__icon,
.empty-panel[data-level="1"] .empty-panel__line,
.empty-panel[data-level="2"] .empty-panel__line {
  display: none;
}
.empty-panel[data-level="1"] .empty-panel__column,
.empty-panel[data-level="2"] .empty-panel__column {
  padding-block: var(--space-3);
}
.empty-panel[data-level="2"] .empty-panel__group {
  display: none;
}
```

- [ ] **Step 6: Run the tests, then the build**

Run: `pnpm test src/features/pane src/features/changes src/features/sessions` then `pnpm build`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/features/pane
git commit -m "fix(pane): a short empty state gives up its mark, its help line and then its list, and never scrolls."
```

---

### Task 5: The pane's tabs are rounded and told apart by fill

**Files:**
- Modify: `src/features/pane/SidePane.css`
- Test: `src/features/pane/SidePane.css.test.ts`

**Interfaces:** none. No markup changes; `SidePane.tsx` is not touched.

- [ ] **Step 1: Write the failing tests**

`SidePane.css.test.ts` (add; `rule(sel)` as in Task 1):

```ts
it("draws a tab as a 24 px rounded shape inside the 28 px strip", () => {
  expect(rule(".side-pane")).toContain("grid-template-rows: 28px minmax(0, 1fr)");
  expect(rule(".side-pane__tab")).toContain("block-size: 24px");
  expect(rule(".side-pane__tab")).toContain("border-radius: var(--radius)");
  expect(rule(".side-pane__tabs")).toContain("align-items: center");
  expect(rule(".side-pane__tabs")).toContain("gap: 2px");
});
it("tells the selected tab by its fill, with no line around it", () => {
  expect(rule('.side-pane__tab[aria-selected="true"]')).toContain("background: var(--fill-strong)");
  expect(rule('.side-pane__tab[aria-selected="true"]')).not.toContain("border");
});
```

The eight existing tests stay and must still pass (96 to 200 px, the forced-colours rules, the hidden scrollbar).

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm test src/features/pane/SidePane.css.test.ts`
Expected: the two new tests FAIL.

- [ ] **Step 3: `SidePane.css`**

- Top comment: the strip holds rounded tabs told apart by fill (Slice 6w decision 10); the selected one is filled, the others are plain with muted text.
- `.side-pane__tabs`: add `align-items: center; gap: 2px;`.
- `.side-pane__tab`: add `block-size: 24px; border-radius: var(--radius);`.
- `.side-pane__tab[aria-selected="true"]`: `background: var(--fill-strong);` in place of `var(--bg-base)`.
- The hover rule for a tab that isn't selected keeps `var(--hover)`. `:active` keeps its inset shadow, which the radius now clips.
- The forced-colors block is unchanged: a selected tab is a `Highlight` fill already.
- The close button (20 px, placed at the tab's end) and the 96 to 200 px widths are unchanged.

- [ ] **Step 4: Run the tests, then the build**

Run: `pnpm test src/features/pane src/components` then `pnpm build`
Expected: all pass, `noBoxedOutlines.test.ts` among them.

- [ ] **Step 5: Commit**

```bash
git add src/features/pane
git commit -m "feat(pane): the pane's tabs are rounded and told apart by fill."
```

---

### Task 6: The live check, its note, and the plan's text

**Files:**
- Create: `docs/research/2026-11-25-files-layout-live-check.md`
- Modify: `docs/PLAN.md` (the Side pane row of the Layout table; "In the file tree" under Keyboard navigation; §9 "Slice 6w decisions" 11)

- [ ] **Step 1: Run the live check** below with the `marshell-live-check` skill. Fix what it finds, each fix with a failing test first and its own commit.
- [ ] **Step 2: Write the note:** per item, what was done and what was seen, and the screenshots' names.
- [ ] **Step 3: Bring `docs/PLAN.md` up to date.**
  - Layout table, Side pane row: "Strip: flat tabs" becomes rounded tabs told apart by fill; "under 480 px of panel width it covers the file instead" becomes "takes the file's place under the path bar"; "A path bar above the file" becomes a path bar across the panel that also holds the open file's tools; the empty state's sentence gains what a short panel gives up.
  - "In the file tree": Space in the narrow layout shows the file and puts the keyboard on the tree button.
  - §9 decision 11: add a "Settled while it was built" list with every ruling the ledger holds.
- [ ] **Step 4: Commit** `docs(research): the Files layout live check, and decision 11 of slice 6w.`

## Live check (exit criteria)

Run with `marshell-live-check` on a fresh scratch folder (scratch `MARSHELL_HOME`, `MARSHELL_DEFAULT_FOLDER`, CLI homes). Under `<Scratch>\home\Marshell\demo` put `src\main.ts` with 60 lines of 200 characters, `README.md`, a PNG, and 12 more files so the recent list can hold 10. The window is resized with the PID-checked `SetWindowPos` helper only, never brought to the front. Screenshots are taken over CDP at each size named.

1. **Wide, one panel (window 1900 and 1200 px).** Files alone in the pane with `main.ts` open: the strip, then one path bar from the panel's start to its end, then the file and the tree. The path bar's bottom line is unbroken across both columns; the divider runs from that line to the bottom; the filter box is inset 8 px on each side with its mark. `document.querySelectorAll(".preview__header").length` is 0 inside Files.
2. **Gutter and status line.** With the tree on the right, a long line of `main.ts` stops 8 px short of the divider, and the status line runs the file column's full width. With "File tree position: Left" the tree, its divider and the code's line numbers read cleanly and there is no gutter rule in force.
3. **Divider.** Dragging it and Left, Right, Home and End on it resize the tree as before; the file keeps 240 px.
4. **Narrow (pane at 320 to 479 px; window about 900 px).** With a file open and the tree button pressed, the tree fills the area under the path bar edge to edge; the path bar, its buttons and the strip are whole; no part of the file or its status line shows. Tab from the filter box and from the last row never lands in the editor. Esc in the tree closes it and the keyboard is in the file. Space on a file shows the file, the keyboard is on the tree button, and Enter there brings the tree back. A click and Enter show the file with the keyboard in it.
5. **Narrow, no file.** Closing the last tab brings the tree back in full. The tree button hides it and shows the empty state whole.
6. **Tools.** `README.md` shows Rendered and Source in the path bar, before "Collapse all folders"; pressing Source swaps the view, and R, Esc and Alt+Left still work with the keyboard on the Source button. The PNG shows its size and Select region there. `main.ts` leaves no gap where the tools were. Switching between the three tabs never leaves two files' tools in the bar. At a 240 px file column with `README.md` open, the tree button is still in view and the path scrolls.
7. **Two and three panels (window 1200 and 900 px).** Items 1 and 4 hold with Browser above Files, and with Changes under them.
8. **Short.** With three panels and no file open, the Files empty state shows "No files open" and both buttons, no mark, no help line, no cut row and no scrollbar; dragging the panel taller brings back the help line and mark, then the whole list, never part of it. Browser's empty state does the same with its ports list. Dragging slowly across the thresholds shows no flicker.
9. **Tabs.** In Browser and Files the selected tab is a filled rounded shape, the others plain; a preview tab's name is italic; the unsaved dot, the new mark and the close button sit inside the shape; dragging a tab and Alt+Shift+Left and Right still move it; the strip is 28 px.
10. **Forced colours and keyboard.** With forced colours emulated over CDP: the selected tab, the filter box and its mark, the divider, the tools and the narrow tree are all readable. Every control of items 1 to 9 is reached with Tab or its key and shows a focus ring.
11. **Right to left** (set `dir="rtl"` on the document over CDP): the tree sits at the left by default, the gutter is on the tree's side, and the path bar's buttons are at its end.

## After the tasks

The whole-branch review (`marshell-reviewer`, with this plan's Review Focus), one fix pass, then `marshell-ship`.
