/** Each value here is a Shiki language id. A test checks that Shiki bundles every one. */

/** By extension (lower-case, no dot). The first group is what the preview has always called code. */
export const EXTENSIONS: Record<string, string> = {
  ts: "typescript", tsx: "tsx", mts: "typescript", cts: "typescript", js: "javascript", jsx: "jsx", mjs: "javascript", cjs: "javascript",
  json: "json", jsonc: "jsonc", json5: "json5", yaml: "yaml", yml: "yaml", toml: "toml", xml: "xml", css: "css", scss: "scss", less: "less",
  rs: "rust", go: "go", py: "python", rb: "ruby", php: "php", java: "java", kt: "kotlin", swift: "swift", c: "c", h: "c", cpp: "cpp",
  cc: "cpp", hpp: "cpp", cs: "csharp", lua: "lua", sh: "bash", bash: "bash", zsh: "bash", ps1: "powershell", psm1: "powershell",
  bat: "bat", cmd: "bat", sql: "sql", graphql: "graphql", vue: "vue", svelte: "svelte", dockerfile: "docker", diff: "diff", patch: "diff",
  ini: "ini", md: "markdown", markdown: "markdown", mdx: "markdown", html: "html", htm: "html",
  // What Sublime Text's default packages open, where Shiki has a grammar.
  pyi: "python", pyw: "python", erb: "erb", rake: "ruby", gemspec: "ruby",
  kts: "kotlin", gradle: "groovy", groovy: "groovy", scala: "scala", sbt: "scala",
  clj: "clojure", cljs: "clojure", cljc: "clojure", edn: "clojure", erl: "erlang", hrl: "erlang", ex: "elixir", exs: "elixir",
  hs: "haskell", lhs: "haskell", ml: "ocaml", mli: "ocaml", fs: "fsharp", fsx: "fsharp", fsi: "fsharp",
  m: "objective-c", mm: "objective-cpp", pl: "perl", pm: "perl", r: "r", d: "d", pas: "pascal",
  lisp: "lisp", el: "emacs-lisp", scm: "scheme", tcl: "tcl", tex: "latex", sty: "latex", cls: "latex", bib: "bibtex",
  rst: "rst", adoc: "asciidoc", plist: "xml", xsd: "xml", xsl: "xml", xslt: "xml", svg: "xml", csproj: "xml",
  sass: "sass", styl: "stylus", hbs: "handlebars", pug: "pug", proto: "proto", tf: "terraform", hcl: "hcl", nix: "nix", zig: "zig",
  dart: "dart", jl: "julia", vim: "vim", fish: "fish", awk: "awk", mk: "make", cmake: "cmake", asm: "asm", s: "asm", vb: "vb",
  gitattributes: "ini", gitmodules: "ini", properties: "properties", reg: "reg", csv: "csv", tsv: "tsv", log: "log", http: "http",
};

/** By whole file name (lower-case). `.env.<anything>` is handled in `languageFor`. */
export const FILE_NAMES: Record<string, string> = {
  makefile: "make", gnumakefile: "make", dockerfile: "docker", containerfile: "docker", "cmakelists.txt": "cmake",
  gemfile: "ruby", rakefile: "ruby", vagrantfile: "ruby", jenkinsfile: "groovy",
  // Shiki has no grammar for these; ini is the closest, and it colours their comments and keys.
  ".gitignore": "ini", ".gitattributes": "ini", ".gitmodules": "ini", ".gitconfig": "ini", ".editorconfig": "ini", ".npmrc": "ini",
  ".env": "dotenv", ".bashrc": "bash", ".bash_profile": "bash", ".zshrc": "bash", ".profile": "bash",
  "cargo.lock": "toml", "pnpm-lock.yaml": "yaml", "tsconfig.json": "jsonc", ".babelrc": "json", ".prettierrc": "json", ".eslintrc": "json",
};

/** By the first line, for a file with no known name or extension. Each pattern is tried in order. */
export const FIRST_LINES: Array<[RegExp, string]> = [
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?(?:sh|bash|zsh|dash)(?=\s|$)/, "bash"],
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?python[\d.]*(?=\s|$)/, "python"],
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?node(?=\s|$)/, "javascript"],
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?ruby[\d.]*(?=\s|$)/, "ruby"],
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?perl[\d.]*(?=\s|$)/, "perl"],
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?php[\d.]*(?=\s|$)/, "php"],
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?pwsh(?=\s|$)/, "powershell"],
  [/^#!\s*(?:\S*\/)?(?:env\s+(?:-\S+\s+)*)?(?:deno|bun)(?=\s|$)/, "typescript"],
  [/^<\?xml/, "xml"],
  [/^<!doctype html/i, "html"],
];

/** The Shiki language of a file: its exact name first, then its extension, then its first line. Undefined for plain text. */
export function languageFor(path: string, firstLine?: string): string | undefined {
  const name = path.slice(Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1).toLowerCase();
  const byName = FILE_NAMES[name];
  if (byName) return byName;
  if (name.startsWith(".env.")) return FILE_NAMES[".env"];
  const dot = name.lastIndexOf(".");
  const byExtension = dot > 0 ? EXTENSIONS[name.slice(dot + 1)] : undefined;
  if (byExtension) return byExtension;
  if (!firstLine) return undefined;
  return FIRST_LINES.find(([pattern]) => pattern.test(firstLine))?.[1];
}
