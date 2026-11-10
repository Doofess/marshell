import { describe, expect, it } from "vitest";
import { SEARCH_URL, addressFor, hostOf, isPageUrl, pageTitle, sameUrl } from "./address";

const open = (url: string) => ({ kind: "open", url });
const search = (words: string) => ({ kind: "search", url: SEARCH_URL + words });
const refused = { kind: "refused" };
const empty = { kind: "empty" };

describe("addressFor", () => {
  it.each([
    ["example.com", open("https://example.com/")],
    ["example.com/a b?q=1", open("https://example.com/a%20b?q=1")],
    ["localhost:3000", open("http://localhost:3000/")],
    ["localhost", open("http://localhost/")],
    ["127.0.0.1:8080/x", open("http://127.0.0.1:8080/x")],
    ["192.168.1.5", open("http://192.168.1.5/")],
    ["[::1]:5173", open("http://[::1]:5173/")],
    ["app.localhost:3000", open("http://app.localhost:3000/")],
    ["https://a.dev/x#y", open("https://a.dev/x#y")],
    ["HTTP://A.dev", open("http://a.dev/")],
    ["about:blank", open("about:blank")],
    ["  example.com  ", open("https://example.com/")],
    ["readme.md", open("https://readme.md/")],
    ["how to center a div", search("how%20to%20center%20a%20div")],
    ["rust", search("rust")],
    ["what is a::b", search("what%20is%20a%3A%3Ab")],
    ["a.b c", search("a.b%20c")],
    ["error: cannot find module", search("error%3A%20cannot%20find%20module")],
    ["c:\\work\\readme.md", refused],
    ["C:/work/x", refused],
    ["c:\\my work\\x y.md", refused],
    ["\\\\server\\share", refused],
    ["/usr/bin", refused],
    ["~/notes.md", refused],
    ["./x", refused],
    ["../x", refused],
    [".\\x", refused],
    ["src/main.rs", refused],
    ["src\\main.rs", refused],
    ["my notes\\todo.txt", refused],
    ["file:///C:/x", refused],
    ["javascript:alert(1)", refused],
    ["javascript:alert(1) ; x", refused],
    ["mailto:a@b.c", refused],
    ["vscode://x", refused],
    ["about:config", refused],
    ["about:blank#x", refused],
    ["data:text/html,x", refused],
    ["http://user:pw@example.com/", refused],
    ["http://user@example.com/", refused],
    ["https://@example.com/", refused],
    ["http:user@example.com", refused],
    ["user:pw@example.com", refused],
    ["user@example.com/x", refused],
    ["user@example.com", refused],
    ["", empty],
    ["   ", empty],
  ])("%j", (input, want) => {
    expect(addressFor(input)).toEqual(want);
  });

  it("a path never becomes a search", () => {
    for (const p of ["C:\\Users\\Ann Lee\\notes.md", "/home/ann/secret project", "~/ann lee", "\\\\srv\\a b"]) {
      expect(addressFor(p).kind).toBe("refused");
    }
  });
});

describe("isPageUrl", () => {
  it.each([
    ["http://x/", true],
    ["https://x/", true],
    ["about:blank", true],
    ["file:///x", false],
    ["http://u@x/", false],
    ["http://u:p@x/", false],
    ["about:config", false],
    ["x", false],
    ["", false],
    ["https://x/" + "a".repeat(4100), false],
  ])("%j", (url, want) => {
    expect(isPageUrl(url)).toBe(want);
  });
});

describe("hostOf, pageTitle and sameUrl", () => {
  it("hostOf gives the host and port, and nothing for a non-URL", () => {
    expect(hostOf("https://example.com/a")).toBe("example.com");
    expect(hostOf("http://localhost:3000/")).toBe("localhost:3000");
    expect(hostOf("about:blank")).toBe("");
    expect(hostOf("nonsense")).toBe("");
  });

  it("pageTitle is the title, else the host, else New tab", () => {
    expect(pageTitle("Docs", "https://a.dev/x")).toBe("Docs");
    expect(pageTitle("", "https://a.dev/x")).toBe("a.dev");
    expect(pageTitle(null, "https://a.dev/x")).toBe("a.dev");
    expect(pageTitle(undefined, "https://a.dev/x")).toBe("a.dev");
    expect(pageTitle(null, "about:blank")).toBe("New tab");
  });

  it("sameUrl ignores a bare host's slash and case, and counts the fragment", () => {
    expect(sameUrl("https://Example.com", "https://example.com/")).toBe(true);
    expect(sameUrl("https://a.dev/x#1", "https://a.dev/x#2")).toBe(false);
    expect(sameUrl("https://a.dev/x", "https://a.dev/y")).toBe(false);
    expect(sameUrl("not a url", "not a url")).toBe(true);
    expect(sameUrl("not a url", "https://a.dev/")).toBe(false);
  });
});
