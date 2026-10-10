// Plays the Departures board's flip cells on the review page. The server markup already holds the final text of every
// cell (and the precomputed frames in data-frames), so the page reads right without this script.
(() => {
  const reducedQuery = matchMedia("(prefers-reduced-motion: reduce)");
  const reducedBox = () => document.getElementById("motion") && document.getElementById("motion").checked;
  const cells = [...document.querySelectorAll(".flip[data-frames]")].map((el) => ({
    el,
    frames: JSON.parse(el.getAttribute("data-frames")),
    at: Number(el.getAttribute("data-at")) || 0,
    final: el.textContent,
  }));
  let timers = [];

  function play() {
    for (const t of timers) cancelAnimationFrame(t);
    timers = [];
    const reduced = reducedQuery.matches || reducedBox();
    for (const c of cells) {
      c.el.textContent = c.final;
      c.el.classList.remove("flip--fade");
      if (reduced) {
        // Reduced motion: no flipping. The cell cross-fades to its new text (the stylesheet's animation), once.
        void c.el.offsetWidth;
        c.el.classList.add("flip--fade");
        continue;
      }
      const start = performance.now() + c.at;
      const step = (now) => {
        const t = now - start;
        if (t < 0) return void (timers[timers.length] = requestAnimationFrame(step));
        let frame = c.frames[0];
        for (const f of c.frames) if (f.t <= t) frame = f;
        c.el.textContent = frame.text;
        if (t < c.frames[c.frames.length - 1].t) timers[timers.length] = requestAnimationFrame(step);
        else c.el.textContent = c.final;
      };
      // Before its moment a cell shows its starting text, so the clip begins from the old state.
      c.el.textContent = c.frames[0].text;
      timers[timers.length] = requestAnimationFrame(step);
    }
  }

  const replay = document.getElementById("replay");
  if (replay) replay.addEventListener("click", play);
  document.querySelectorAll("button").forEach((b) => {
    if (b.textContent.trim() === "Replay") b.addEventListener("click", play);
  });
  document.getElementById("motion")?.addEventListener("change", play);
  if (cells.length) play();
})();
