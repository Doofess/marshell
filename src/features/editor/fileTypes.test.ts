import { bundledLanguages } from "shiki/langs";
import { describe, expect, it } from "vitest";
import { EXTENSIONS, FILE_NAMES, FIRST_LINES, languageFor } from "./fileTypes";

/** What the preview's old table said, so the move to this table loses nothing. */
const OLD_TABLE: Record<string, string> = {
  ts: "typescript", tsx: "tsx", mts: "typescript", cts: "typescript", js: "javascript", jsx: "jsx", mjs: "javascript", cjs: "javascript",
  json: "json", jsonc: "jsonc", json5: "json5", yaml: "yaml", yml: "yaml", toml: "toml", xml: "xml", css: "css", scss: "scss", less: "less",
  rs: "rust", go: "go", py: "python", rb: "ruby", php: "php", java: "java", kt: "kotlin", swift: "swift", c: "c", h: "c", cpp: "cpp",
  cc: "cpp", hpp: "cpp", cs: "csharp", lua: "lua", sh: "bash", bash: "bash", zsh: "bash", ps1: "powershell", psm1: "powershell",
  bat: "bat", cmd: "bat", sql: "sql", graphql: "graphql", vue: "vue", svelte: "svelte", dockerfile: "docker", diff: "diff", patch: "diff",
  ini: "ini", md: "markdown", markdown: "markdown", mdx: "markdown", html: "html", htm: "html",
};

describe("fileTypes", () => {
  it("every_language_in_the_table_is_one_shiki_bundles", () => {
    const ids = new Set(Object.keys(bundledLanguages));
    const values = [...Object.values(EXTENSIONS), ...Object.values(FILE_NAMES), ...FIRST_LINES.map(([, id]) => id)];
    expect(values.filter((id) => !ids.has(id))).toEqual([]);
  });
  it("the_old_table_is_all_still_there", () => {
    for (const [ext, id] of Object.entries(OLD_TABLE)) expect(languageFor(`a.${ext}`), ext).toBe(id);
  });
  it("covers the extensions the brief lists", () => {
    const listed =
      "mts cts pyi pyw erb rake gemspec kts gradle groovy scala sbt clj cljs cljc edn erl hrl ex exs hs lhs ml mli fs fsx fsi m mm pl pm r d pas lisp el scm tcl tex sty cls bib rst adoc yaml yml plist xsd xsl xslt svg sass styl hbs pug proto tf hcl nix zig dart jl vim fish awk mk cmake asm s vb csproj gitattributes gitmodules properties reg csv tsv log http".split(
        " ",
      );
    expect(listed.filter((ext) => !EXTENSIONS[ext])).toEqual([]);
  });
  it("a_file_name_beats_its_extension", () => {
    expect(languageFor("CMakeLists.txt")).toBe("cmake");
    expect(languageFor("tsconfig.json")).toBe("jsonc");
    expect(languageFor("Cargo.lock")).toBe("toml");
    expect(languageFor("pnpm-lock.yaml")).toBe("yaml");
    expect(languageFor(".babelrc")).toBe("json");
    expect(languageFor(".gitignore")).toBeDefined();
  });
  it("names_match_in_any_case_and_with_any_folder", () => {
    expect(languageFor(String.raw`C:\x\MAKEFILE`)).toBe("make");
    expect(languageFor("/a/b/Dockerfile")).toBe("docker");
    expect(languageFor("/a/b/GNUmakefile")).toBe("make");
  });
  it("dot_env_variants", () => {
    expect(languageFor(".env")).toBeDefined();
    expect(languageFor(".env.local")).toBe(languageFor(".env"));
    expect(languageFor("/p/.env.production")).toBe(languageFor(".env"));
  });
  it("a_first_line_names_the_language", () => {
    expect(languageFor("run", "#!/bin/sh")).toBe("bash");
    expect(languageFor("run", "#!/usr/bin/env bash")).toBe("bash");
    expect(languageFor("run", "#!/bin/zsh -e")).toBe("bash");
    expect(languageFor("run", "#!/usr/bin/dash")).toBe("bash");
    expect(languageFor("run", "#!/usr/bin/python3")).toBe("python");
    expect(languageFor("run", "#!/usr/bin/env python -u")).toBe("python");
    expect(languageFor("run", "#!/usr/bin/env node")).toBe("javascript");
    expect(languageFor("run", "#!/usr/bin/ruby")).toBe("ruby");
    expect(languageFor("run", "#!/usr/bin/perl -w")).toBe("perl");
    expect(languageFor("run", "#!/usr/bin/env php")).toBe("php");
    expect(languageFor("run", "#!/usr/bin/env pwsh")).toBe("powershell");
    expect(languageFor("run", "#!/usr/bin/env deno run")).toBe("typescript");
    expect(languageFor("run", "#!/usr/bin/env bun")).toBe("typescript");
    expect(languageFor("run", '<?xml version="1.0"?>')).toBe("xml");
    expect(languageFor("run", "<!DOCTYPE html>")).toBe("html");
    expect(languageFor("run", "<!doctype HTML>")).toBe("html");
  });
  it("an_extension_beats_the_first_line", () => {
    expect(languageFor("a.py", "#!/bin/bash")).toBe("python");
  });
  it("nothing_known_is_undefined", () => {
    expect(languageFor("notes")).toBeUndefined();
    expect(languageFor("a.xyz")).toBeUndefined();
    expect(languageFor("notes", "")).toBeUndefined();
    expect(languageFor("notes", "just words")).toBeUndefined();
  });
});
