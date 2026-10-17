import { describe, expect, it } from "vitest";
import { CSP, buildSrcdoc } from "./htmlDoc";

describe("buildSrcdoc", () => {
  it("starts with the CSP meta that denies everything but inline styles and data images", () => {
    const doc = buildSrcdoc("<p>hi</p>");
    expect(CSP).toBe("default-src 'none'; style-src 'unsafe-inline'; img-src data:");
    expect(doc.startsWith(`<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}">`)).toBe(true);
  });
  it("puts the CSP before any meta, base or link of the file", () => {
    const doc = buildSrcdoc('<meta charset="x"><base href="http://evil/"><link rel="stylesheet" href="http://evil/a.css"><meta name="a" content="b">');
    const csp = doc.indexOf("Content-Security-Policy");
    expect(csp).toBeGreaterThan(0);
    for (const re of [/<meta\s+name/i, /<link\b/i]) expect(doc.search(re), String(re)).toBeGreaterThan(csp);
    // Everything before the CSP meta is ours: a doctype and the charset tag.
    expect(doc.slice(0, csp).match(/<meta/g)).toHaveLength(2);
    expect(doc).not.toMatch(/<base/i);
  });
  it("removes a refresh meta however it is written", () => {
    const variants = [
      '<meta http-equiv="refresh" content="0;url=http://evil/">',
      "<meta http-equiv='refresh' content='0;url=http://evil/'>",
      "<meta http-equiv=refresh content=0;url=http://evil/>",
      '<META HTTP-EQUIV="Refresh" CONTENT="0;url=http://evil/">',
      '<meta content="0;url=http://evil/" http-equiv="refresh">',
      '<meta content="a>b" http-equiv=refresh>',
      '<meta\n  http-equiv = "REFRESH"\n  content="1">',
      '<meta/http-equiv="refresh" content="1">',
      '<meta http-equiv="refresh" content="1"/>',
    ];
    for (const v of variants) {
      const doc = buildSrcdoc(`<p>before</p>${v}<p>after</p>`);
      expect(doc, v).not.toMatch(/refresh/i);
      expect(doc, v).toContain("<p>before</p><p>after</p>");
    }
  });
  it("removes base tags", () => {
    const doc = buildSrcdoc('<head><base href="http://evil/"><BASE target=_top></head><p>x</p>');
    expect(doc).not.toMatch(/<base/i);
    expect(doc).toContain("<head></head><p>x</p>");
  });
  // What follows the injected policy: our own meta has an http-equiv, so only the part after it is the file's.
  const fileHalf = (doc: string) => doc.slice(doc.indexOf(CSP) + CSP.length);
  it("is not fooled by a tag nested inside a tag, however deep", () => {
    const refresh = '<meta http-equiv=refresh content="0;url=https://x">';
    const nest = (n: number) => {
      let s = refresh;
      for (let i = 0; i < n; i++) s = `<me<meta http-equiv=a>${s.slice(3)}`;
      return s;
    };
    const inputs = [
      '<me<me<meta http-equiv=a>ta http-equiv=b>ta http-equiv=refresh content="0;url=https://x">',
      "<ba<base href=x>se href=https://x>",
      "<ba<ba<base href=x>se href=y>se href=https://x>",
      nest(3),
      nest(40),
    ];
    for (const input of inputs) {
      const file = fileHalf(buildSrcdoc(`<p>a</p>${input}<p>b</p>`));
      expect(file, input).not.toMatch(/http-equiv/i);
      expect(file, input).not.toMatch(/<base/i);
      expect(file, input).not.toMatch(/<meta[^>]*refresh/i);
    }
  });
  it("leaves the rest of the file as it is", () => {
    const body = '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><style>p{color:red}</style></head><body><p>hi</p><a href="#x">x</a></body></html>';
    expect(buildSrcdoc(body).endsWith(body)).toBe(true);
  });
});
