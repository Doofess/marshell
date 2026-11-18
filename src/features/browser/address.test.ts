import { describe, expect, it } from "vitest";
import cases from "../../../src-tauri/src/browser/allowed.json";
import { SEARCH_URL, addressFor, hostOf, isPageUrl, ownOrigins, pageTitle, sameUrl, shellRefuses } from "./address";

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

describe("shellRefuses", () => {
  it("shellRefuses_agrees_with_the_shell_in_every_shared_case", () => {
    expect(cases.length).toBeGreaterThanOrEqual(40);
    for (const c of cases) expect(shellRefuses(c.url, c.own as [string, number][]), `${c.url} with ${JSON.stringify(c.own)}`).toBe(!c.allowed);
  });
  it("refuses an address that does not parse", () => {
    expect(shellRefuses("not a url", [])).toBe(true);
    expect(shellRefuses("", [])).toBe(true);
  });
});

describe("ownOrigins", () => {
  it("an_installed_build_names_only_the_api_port", () => {
    const own = ownOrigins({ protocol: "http:", hostname: "tauri.localhost", port: "" }, 51000, false);
    expect(own).toEqual([["127.0.0.1", 51000]]);
    for (const url of ["http://localhost/", "http://127.0.0.1/", "http://myapp.localhost/", "http://[::1]/"]) expect(shellRefuses(url, own), url).toBe(false);
    for (const url of ["http://127.0.0.1:51000/", "http://tauri.localhost/"]) expect(shellRefuses(url, own), url).toBe(true);
  });
  it("the_dev_server_is_named_under_tauri_dev", () => {
    const own = ownOrigins({ protocol: "http:", hostname: "localhost", port: "1420" }, 51000, true);
    expect(own).toEqual([["localhost", 1420], ["127.0.0.1", 51000]]);
    expect(shellRefuses("http://127.0.0.1:1420/", own)).toBe(true);
  });
  it("names_no_api_entry_without_a_port", () => {
    expect(ownOrigins({ protocol: "http:", hostname: "localhost", port: "1420" }, null, true)).toEqual([["localhost", 1420]]);
    expect(ownOrigins({ protocol: "https:", hostname: "x.dev", port: "" }, null, true)).toEqual([["x.dev", 443]]);
    expect(ownOrigins({ protocol: "tauri:", hostname: "localhost", port: "" }, null, true)).toEqual([]);
  });
});
