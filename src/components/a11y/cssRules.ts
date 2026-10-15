/** A tiny CSS reader for the lint tests: rules with their selectors, with at-rules walked into. */
type Rule = { selectors: string[]; body: string; forced: boolean };

/** Rules of a stylesheet, comments stripped, with at-rules (media, supports) walked into by matching braces. */
export function rules(css: string): Rule[] {
  const out: Rule[] = [];
  const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const walkBlock = (src: string, forced: boolean) => {
    let i = 0;
    while (i < src.length) {
      const open = src.indexOf("{", i);
      if (open < 0) return;
      const head = src.slice(i, open).trim();
      let depth = 0;
      let j = open;
      for (; j < src.length; j++) {
        if (src[j] === "{") depth++;
        else if (src[j] === "}" && --depth === 0) break;
      }
      const inner = src.slice(open + 1, j);
      if (head.startsWith("@")) walkBlock(inner, forced || /forced-colors:\s*active/.test(head));
      else out.push({ selectors: head.split(",").map((s) => s.trim()), body: inner, forced });
      i = j + 1;
    }
  };
  walkBlock(text, false);
  return out;
}
