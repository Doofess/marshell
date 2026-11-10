import { describe, expect, it, vi } from "vitest";
import { pathLinkProvider, type BufferLike } from "./pathLinks";
import { findUrls, targetHint, urlHint, urlLinkProvider, webLinkHandler } from "./urlLinks";

/** The same stub as pathLinks.test.ts: a row starting with `!` is a wrapped continuation; `世` is wide and followed by `~`. */
function stub(rows: string[]): BufferLike {
  return {
    getLine: (y) => {
      const raw = rows[y];
      if (raw === undefined) return undefined;
      const isWrapped = raw.startsWith("!");
      const cells = Array.from(isWrapped ? raw.slice(1) : raw).map((ch): [string, number] => {
        if (ch === "~") return ["", 0];
        return [ch, ch === "世" ? 2 : 1];
      });
      return {
        isWrapped,
        length: cells.length,
        getCell: (x) => {
          const cell = cells[x];
          return cell ? { getChars: () => cell[0], getWidth: () => cell[1] } : undefined;
        },
      };
    },
  };
}

const urls = (text: string) => findUrls(text).map((u) => u.url);

describe("findUrls", () => {
  it("finds http and https addresses and nothing else", () => {
    expect(urls("a http://example.com b HTTPS://Example.com/x c")).toEqual(["http://example.com", "HTTPS://Example.com/x"]);
    expect(urls("file:///c:/x javascript:alert(1) ftp://h/x www.example.com localhost:3000 vscode://x/y")).toEqual([]);
  });

  it("leaves out a full stop, comma or bracket that ends the sentence", () => {
    expect(urls("see https://example.com/a.")).toEqual(["https://example.com/a"]);
    expect(urls("see https://example.com/a, then")).toEqual(["https://example.com/a"]);
    expect(urls("(https://example.com/a)")).toEqual(["https://example.com/a"]);
    expect(urls("[https://example.com/a]")).toEqual(["https://example.com/a"]);
    expect(urls("https://en.wikipedia.org/wiki/Rust_(language)")).toEqual(["https://en.wikipedia.org/wiki/Rust_(language)"]);
    expect(urls("(https://en.wikipedia.org/wiki/Rust_(language))")).toEqual(["https://en.wikipedia.org/wiki/Rust_(language)"]);
    for (const tail of ['"', "'", ">", "]", ";", ":", "!", "?", ".)", "?!", "'.", "}"]) {
      expect(urls(`https://example.com/a${tail}`)).toEqual(["https://example.com/a"]);
    }
    expect(urls('<a href="https://example.com/a">')).toEqual(["https://example.com/a"]);
  });

  it("keeps a port, a query and a fragment", () => {
    expect(urls("go to http://127.0.0.1:5173/a?b=c#d now")).toEqual(["http://127.0.0.1:5173/a?b=c#d"]);
  });

  it("finds two addresses on one line, with their offsets", () => {
    const text = "x https://a.example/1 and http://b.example/2.";
    expect(findUrls(text)).toEqual([
      { start: 2, end: 21, url: "https://a.example/1" },
      { start: 26, end: 44, url: "http://b.example/2" },
    ]);
  });

  it("an address longer than 2048 characters is not a link", () => {
    expect(urls(`https://example.com/${"a".repeat(2048 - 20)}`)).toHaveLength(1);
    expect(urls(`https://example.com/${"a".repeat(2048 - 19)}`)).toEqual([]);
  });

  it("needs a host", () => {
    expect(urls("https://")).toEqual([]);
    expect(urls("see https://. and http://, ok")).toEqual([]);
  });

  it("counts offsets in UTF-16 units", () => {
    const [u] = findUrls("😀 https://a.example");
    expect(u).toEqual({ start: 3, end: 20, url: "https://a.example" });
  });
});

describe("urlLinkProvider", () => {
  const term = (rows: string[]) => ({ buffer: { active: stub(rows) }, element: undefined });
  const run = (p: ReturnType<typeof urlLinkProvider>, y: number) => new Promise<Parameters<Parameters<typeof p.provideLinks>[1]>[0]>((r) => p.provideLinks(y, r));

  it("gives a range over the address's cells (1-based, end inclusive)", async () => {
    const links = (await run(urlLinkProvider(term(["see https://a.example/x ok"]), "linux"), 1)) ?? [];
    expect(links).toHaveLength(1);
    expect(links[0]!.text).toBe("https://a.example/x");
    expect(links[0]!.range).toEqual({ start: { x: 5, y: 1 }, end: { x: 23, y: 1 } });
  });

  it("maps an address after a wide character to the right cells", async () => {
    // "世" takes x 1-2, the space is x 3, so the address starts at x 4.
    const links = (await run(urlLinkProvider(term(["世~ https://a.example"]), "linux"), 1)) ?? [];
    expect(links[0]!.range).toEqual({ start: { x: 4, y: 1 }, end: { x: 20, y: 1 } });
  });

  it("a wrapped address is one link across its rows", async () => {
    const rows = ["go https://exam", "!ple.com/a/b ok"];
    const p = urlLinkProvider(term(rows), "linux");
    for (const y of [1, 2]) {
      const links = (await run(p, y)) ?? [];
      expect(links).toHaveLength(1);
      expect(links[0]!.text).toBe("https://example.com/a/b");
      expect(links[0]!.range).toEqual({ start: { x: 4, y: 1 }, end: { x: 11, y: 2 } });
    }
  });

  it("gives no link on a row the address does not reach", async () => {
    expect(await run(urlLinkProvider(term(["plain", "https://a.example"]), "linux"), 1)).toBeUndefined();
  });

  it("only Ctrl+click opens it (Cmd on macOS)", async () => {
    const open = vi.fn(async () => {});
    const [l] = (await run(urlLinkProvider(term(["https://a.example/x?y=1"]), "linux", open), 1)) ?? [];
    l!.activate({ ctrlKey: false, metaKey: false } as MouseEvent, l!.text);
    l!.activate({ ctrlKey: false, metaKey: true } as MouseEvent, l!.text);
    expect(open).not.toHaveBeenCalled();
    l!.activate({ ctrlKey: true, metaKey: false } as MouseEvent, l!.text);
    expect(open).toHaveBeenCalledTimes(1);
    expect(open).toHaveBeenCalledWith("https://a.example/x?y=1");

    const mac = vi.fn(async () => {});
    const [m] = (await run(urlLinkProvider(term(["https://a.example"]), "macos", mac), 1)) ?? [];
    m!.activate({ ctrlKey: true, metaKey: false } as MouseEvent, m!.text);
    expect(mac).not.toHaveBeenCalled();
    m!.activate({ ctrlKey: false, metaKey: true } as MouseEvent, m!.text);
    expect(mac).toHaveBeenCalledWith("https://a.example");
  });

  it("swallows a rejected open", async () => {
    const open = vi.fn(async () => {
      throw new Error("no browser");
    });
    const [l] = (await run(urlLinkProvider(term(["https://a.example"]), "linux", open), 1)) ?? [];
    expect(() => l!.activate({ ctrlKey: true } as MouseEvent, l!.text)).not.toThrow();
    await Promise.resolve();
    expect(open).toHaveBeenCalledTimes(1);
  });

  it("sets the hint as the element's title on hover and clears it on leave", async () => {
    const element = { setAttribute: vi.fn(), removeAttribute: vi.fn() };
    const p = urlLinkProvider({ buffer: { active: stub(["https://a.example"]) }, element: element as unknown as HTMLElement }, "linux");
    const [l] = (await run(p, 1)) ?? [];
    l!.hover!({} as MouseEvent, l!.text);
    expect(element.setAttribute).toHaveBeenCalledWith("title", "Ctrl+click to open in your browser");
    l!.leave!({} as MouseEvent, l!.text);
    expect(element.removeAttribute).toHaveBeenCalledWith("title");
  });

  it("the hint names the browser", () => {
    expect(urlHint("windows")).toBe("Ctrl+click to open in your browser");
    expect(urlHint("linux")).toBe("Ctrl+click to open in your browser");
    expect(urlHint("macos")).toBe("⌘+click to open in your browser");
  });

  it("a path-like stretch inside an address is one web link and not also a path link", async () => {
    const t = term(["https://example.com/src/main.rs:12 and src/main.rs"]);
    const resolve = vi.fn(async (_tab: string, c: string[]) => c.map((x) => `/p/${x}`));
    const web = (await run(urlLinkProvider(t, "linux"), 1)) ?? [];
    const paths = (await run(pathLinkProvider(t, "t1", "linux", vi.fn(), resolve), 1)) ?? [];
    expect(web.map((l) => l.text)).toEqual(["https://example.com/src/main.rs:12"]);
    // The path provider is asked only about the separate path, never about a piece of the address.
    expect(resolve).toHaveBeenCalledWith("t1", ["src/main.rs"]);
    expect(paths.map((l) => l.range.start.x)).toEqual([40]);
  });
});

describe("webLinkHandler (OSC 8 hyperlinks)", () => {
  it("opens an http or https target only with the modifier", () => {
    const open = vi.fn(async () => {});
    const h = webLinkHandler("linux", () => undefined, open);
    const range = { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } };
    h.activate({ ctrlKey: false, metaKey: false } as MouseEvent, "https://a.example/", range);
    expect(open).not.toHaveBeenCalled();
    h.activate({ ctrlKey: true } as MouseEvent, "https://a.example/", range);
    h.activate({ ctrlKey: true } as MouseEvent, "http://a.example/x", range);
    expect(open.mock.calls).toEqual([["https://a.example/"], ["http://a.example/x"]]);
  });

  it("does nothing for any other target", () => {
    const open = vi.fn(async () => {});
    const h = webLinkHandler("linux", () => undefined, open);
    const range = { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } };
    for (const target of ["file:///c:/x", "javascript:alert(1)", "vscode://x/y", "ftp://h/x", "not a url", ""]) {
      h.activate({ ctrlKey: true } as MouseEvent, target, range);
    }
    expect(open).not.toHaveBeenCalled();
  });

  it("the hint over a hyperlink names the address it opens, whatever its text says", () => {
    const element = { setAttribute: vi.fn(), removeAttribute: vi.fn() };
    const h = webLinkHandler("windows", () => element as unknown as HTMLElement, vi.fn(async () => {}));
    const range = { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } };
    h.hover!({} as MouseEvent, "https://a.example/x?y=1", range);
    expect(element.setAttribute).toHaveBeenCalledWith("title", "Ctrl+click to open https://a.example/x?y=1 in your browser");
    expect(targetHint("linux", "http://127.0.0.1:8080/")).toBe("Ctrl+click to open http://127.0.0.1:8080/ in your browser");
  });

  it("a long address, or one with a user name before the host, is named by where it really goes", () => {
    expect(targetHint("windows", "https://good.example@evil.example/login")).toBe("Ctrl+click to open https://evil.example/… in your browser");
    expect(targetHint("windows", "https://good.example:pw@evil.example:8443/")).toBe("Ctrl+click to open https://evil.example:8443/… in your browser");
    expect(targetHint("windows", `https://a.example/${"x".repeat(200)}`)).toBe("Ctrl+click to open https://a.example/… in your browser");
    expect(targetHint("windows", `https://a.example/${"x".repeat(100)}`)).toBe(`Ctrl+click to open https://a.example/${"x".repeat(100)} in your browser`);
  });

  it("the hint shows the address as the browser will read it, not as the program wrote it", () => {
    // A Cyrillic "а" in the host shows as punycode, a direction override in the path as its escape.
    expect(targetHint("windows", "https://аpple.example/x")).toBe("Ctrl+click to open https://xn--pple-43d.example/x in your browser");
    expect(targetHint("windows", "https://a.example/‮gpj.exe")).toBe("Ctrl+click to open https://a.example/%E2%80%AEgpj.exe in your browser");
    expect(targetHint("windows", "HTTPS://A.Example")).toBe("Ctrl+click to open https://a.example/ in your browser");
    expect(targetHint("windows", "https://good.example\\@evil.example/")).toBe("Ctrl+click to open https://good.example/@evil.example/ in your browser");
  });

  it("a hyperlink opens the address its hint showed", () => {
    const open = vi.fn(async () => {});
    const h = webLinkHandler("windows", () => undefined, open);
    const range = { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } };
    h.activate({ ctrlKey: true } as MouseEvent, "HTTPS://A.Example", range);
    h.activate({ ctrlKey: true } as MouseEvent, "https://аpple.example/x", range);
    expect(open.mock.calls).toEqual([["https://a.example/"], ["https://xn--pple-43d.example/x"]]);
  });

  it("a target that would not open gets no hint", () => {
    const element = { setAttribute: vi.fn(), removeAttribute: vi.fn() };
    const h = webLinkHandler("windows", () => element as unknown as HTMLElement, vi.fn(async () => {}));
    const range = { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } };
    for (const target of ["file:///c:/x", "javascript:alert(1)", "not a url", ""]) {
      expect(targetHint("windows", target)).toBeNull();
      h.hover!({} as MouseEvent, target, range);
    }
    expect(element.setAttribute).not.toHaveBeenCalled();
    expect(element.removeAttribute).toHaveBeenCalledWith("title");
  });

  it("uses the Cmd key on macOS", () => {
    const open = vi.fn(async () => {});
    const element = { setAttribute: vi.fn(), removeAttribute: vi.fn() };
    const h = webLinkHandler("macos", () => element as unknown as HTMLElement, open);
    const range = { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } };
    h.activate({ ctrlKey: true, metaKey: false } as MouseEvent, "https://a.example/", range);
    expect(open).not.toHaveBeenCalled();
    h.activate({ ctrlKey: false, metaKey: true } as MouseEvent, "https://a.example/", range);
    expect(open).toHaveBeenCalledWith("https://a.example/");
    h.hover!({} as MouseEvent, "https://a.example/", range);
    expect(element.setAttribute).toHaveBeenCalledWith("title", "⌘+click to open https://a.example/ in your browser");
    h.leave!({} as MouseEvent, "https://a.example/", range);
    expect(element.removeAttribute).toHaveBeenCalledWith("title");
  });
});
