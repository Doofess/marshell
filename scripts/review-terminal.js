// Mounts a real xterm.js into every .scripted-terminal on the review page and applies the page's terminal controls.
(() => {
  const data = JSON.parse(document.getElementById("terminal-data").textContent);
  const mounted = [];

  // Same rule as schemeFromComputed in src/features/terminal/terminalTheme.ts.
  function schemeOf(el) {
    const cs = getComputedStyle(el).colorScheme;
    if (cs === "dark" || cs === "light") return cs;
    return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function options(el) {
    const select = document.getElementById("terminal-theme");
    const setting = el.dataset.setting !== "follow-app" ? el.dataset.setting : select.value;
    const scheme = setting === "follow-app" ? schemeOf(el) : setting;
    const protect = document.getElementById("terminal-contrast").checked;
    return { theme: data.palettes[scheme], minimumContrastRatio: protect ? data.minContrast : 1 };
  }

  function mount(el) {
    const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
    const term = new Terminal({ fontFamily, fontSize: 13, cols: data.size.cols, rows: data.size.rows, disableStdin: true, cursorBlink: false, scrollback: 1000, ...options(el) });
    const fit = new FitAddon.FitAddon();
    term.loadAddon(fit);
    term.open(el);
    term.write(data.session);
    fit.fit();
    new ResizeObserver(() => fit.fit()).observe(el);
    el.style.backgroundColor = options(el).theme.background;
    mounted.push({ el, term });
  }

  function apply() {
    for (const { el, term } of mounted) {
      const o = options(el);
      term.options.theme = o.theme;
      term.options.minimumContrastRatio = o.minimumContrastRatio;
      el.style.backgroundColor = o.theme.background;
    }
  }

  document.querySelectorAll(".scripted-terminal").forEach(mount);
  document.getElementById("terminal-theme").addEventListener("change", apply);
  document.getElementById("terminal-contrast").addEventListener("change", apply);
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", apply);
})();
