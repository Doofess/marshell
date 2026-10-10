// In-page accessibility checks for the review page. Plain browser script: the Playwright runner (scripts/a11y-matrix.mjs) and
// anyone with a browser console or the Playwright MCP's browser_evaluate can use it the same way:
//   window.a11yChecks()  ->  { mode, findings: [{ rule, where, detail }], counts }
// It only reads; it moves focus (and puts it back) for the focus-ring check.
(() => {
  const PAGE = ".window, .launcher, .palette";
  const where = (el) => {
    const id = el.id ? `#${el.id}` : "";
    const cls = typeof el.className === "string" && el.className ? "." + el.className.trim().split(/\s+/).slice(0, 2).join(".") : "";
    const section = el.closest("section[id]")?.id;
    return `${section ? section + " " : ""}${el.tagName.toLowerCase()}${id}${cls}`;
  };
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && cs.visibility !== "hidden" && cs.display !== "none";
  };
  const hasOutline = (cs) => cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) > 0 && !/rgba\(\d+, \d+, \d+, 0\)|transparent/.test(cs.outlineColor);

  window.a11yChecks = () => {
    const forced = matchMedia("(forced-colors: active)").matches;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const findings = [];
    const add = (rule, el, detail) => findings.push({ rule, where: where(el), detail });
    const counts = {};

    // 1. In forced colours a selected item must still be marked: fills and shadows are gone, so it needs an outline, or a
    //    background that differs from the page's own (a system-colour fill).
    const selected = [...document.querySelectorAll(`:is(${PAGE.split(", ").map((s) => s + " ").join(", ")}) :is([aria-selected="true"], [aria-current="true"], [data-selected="true"])`)].filter(visible);
    counts.selected = selected.length;
    if (forced)
      for (const el of selected) {
        const cs = getComputedStyle(el);
        const host = el.closest(PAGE);
        const base = host ? getComputedStyle(host).backgroundColor : "rgba(0, 0, 0, 0)";
        if (!hasOutline(cs) && cs.backgroundColor === base) add("forced-colors-selected", el, "selected but no outline and no distinct fill in forced colours");
      }

    // 2. Every control shows where keyboard focus is. Focus each one in turn, keep the previous focus, and require an outline.
    const previous = document.activeElement;
    const controls = [...document.querySelectorAll(`:is(${PAGE.split(", ").map((s) => s + " ").join(", ")}) :is(button, [role="option"][tabindex], [role="combobox"], input, textarea, select, a[href])`)].filter(visible);
    counts.focusable = controls.length;
    const seen = new Set();
    for (const el of controls) {
      const key = where(el);
      if (seen.has(key)) continue; // one sample per kind of control is enough
      seen.add(key);
      if (el.disabled || el.getAttribute("aria-disabled") === "true") continue;
      // Focus may be drawn by the control itself (an outline) or by its bar (:focus-within changes the container), so compare the
      // control and three ancestors before and after focusing it.
      const chain = [el, el.parentElement, el.parentElement?.parentElement, el.parentElement?.parentElement?.parentElement].filter(Boolean);
      const snap = () => chain.map((n) => { const cs = getComputedStyle(n); const pb = getComputedStyle(n, "::before"); const pa = getComputedStyle(n, "::after"); return [cs.outlineStyle, cs.outlineColor, cs.boxShadow, cs.backgroundColor, cs.backgroundImage, cs.borderTopColor, cs.borderBottomColor, pb.backgroundColor, pb.opacity, pa.backgroundColor, pa.opacity].join("|"); });
      if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
      const before = snap();
      el.focus({ focusVisible: true });
      if (document.activeElement !== el) continue;
      const after = snap();
      if (!hasOutline(getComputedStyle(el)) && before.join("~") === after.join("~") && !el.matches(".xterm *")) add("focus-visible", el, "keyboard focus changes nothing visible on the control or its bar");
    }
    if (previous && previous.focus) previous.focus();
    else if (document.activeElement && document.activeElement.blur) document.activeElement.blur();

    // 3. Text clipped without an ellipsis (what browser zoom breaks first): a clipped box whose content is wider than it is.
    let clipped = 0;
    for (const el of document.querySelectorAll(`:is(${PAGE.split(", ").map((s) => s + " ").join(", ")}) *`)) {
      if (el.closest(".xterm, .scripted-terminal")) continue;
      const cs = getComputedStyle(el);
      if (!/clip|hidden/.test(cs.overflowX)) continue;
      if (el.scrollWidth <= el.clientWidth + 1 || el.clientWidth <= 1) continue; // <= 1: a visually hidden screen-reader-only box is meant to be clipped
      if (cs.textOverflow === "ellipsis" || cs.display === "flex" || cs.display === "grid" || cs.display === "inline-flex") continue;
      if (!el.textContent || !el.textContent.trim()) continue;
      clipped++;
      add("clipped-text", el, `content ${el.scrollWidth}px in ${el.clientWidth}px with no ellipsis`);
    }
    counts.clipped = clipped;

    // 4. Reduced motion: nothing loops.
    if (reduced) {
      const looping = document.getAnimations().filter((a) => a.playState === "running" && a.effect && a.effect.getComputedTiming().iterations === Infinity);
      counts.loopingAnimations = looping.length;
      for (const a of looping.slice(0, 20)) add("reduced-motion", a.effect.target || document.body, `${a.animationName || a.transitionProperty || "animation"} still loops`);
    }

    // 5. One live region per window.
    for (const win of document.querySelectorAll(".window")) {
      const live = win.querySelectorAll('[role="status"], [role="alert"], [aria-live]:not([aria-live="off"])').length;
      if (live > 1) add("live-regions", win, `${live} live regions in one window`);
    }

    return { mode: { forcedColors: forced, reducedMotion: reduced, width: innerWidth, dpr: devicePixelRatio }, counts, findings };
  };
})();
