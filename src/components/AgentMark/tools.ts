/**
 * Manual CLIs: programs a person types into themselves (shells, dev tools, remote and cloud tools). They get a mark like
 * the AI vendors do, but it is always still: no working loop, no context ring, no model, no prompt line. Marshell would
 * pick the mark from the session's foreground process (docs/IDEAS.md), falling back to `program` for anything unknown.
 *
 * Phase 0 marks are simple glyphs drawn here on the 24-unit grid with `fill-rule: evenodd`. They stand in for the tools'
 * own logos, which are trademarks of their owners and must be sourced and checked (for example from Simple Icons, CC0)
 * before release; they appear only to say which program a session runs.
 */
export const TOOL_GROUPS = ["shell", "dev", "remote", "fallback"] as const;
export type ToolGroup = (typeof TOOL_GROUPS)[number];

export const TOOL_GROUP_NAMES: Record<ToolGroup, string> = {
  shell: "Shells",
  dev: "Dev tools",
  remote: "Remote and cloud",
  fallback: "Anything else",
};

export const TOOL_IDS = [
  "powershell",
  "bash",
  "zsh",
  "cmd",
  "gitbash",
  "wsl",
  "git",
  "docker",
  "kubectl",
  "node",
  "python",
  "cargo",
  "gh",
  "terraform",
  "ssh",
  "aws",
  "gcloud",
  "az",
  "supabase",
  "vercel",
  "program",
] as const;
export type ToolId = (typeof TOOL_IDS)[number];

export const TOOL_NAMES: Record<ToolId, string> = {
  powershell: "PowerShell",
  bash: "Bash",
  zsh: "Zsh",
  cmd: "Command Prompt",
  gitbash: "Git Bash",
  wsl: "WSL",
  git: "Git",
  docker: "Docker",
  kubectl: "kubectl",
  node: "Node.js",
  python: "Python",
  cargo: "Cargo",
  gh: "GitHub CLI",
  terraform: "Terraform",
  ssh: "SSH",
  aws: "AWS CLI",
  gcloud: "Google Cloud CLI",
  az: "Azure CLI",
  supabase: "Supabase CLI",
  vercel: "Vercel CLI",
  program: "Terminal program",
};

export const TOOL_GROUP: Record<ToolId, ToolGroup> = {
  powershell: "shell",
  bash: "shell",
  zsh: "shell",
  cmd: "shell",
  gitbash: "shell",
  wsl: "shell",
  git: "dev",
  docker: "dev",
  kubectl: "dev",
  node: "dev",
  python: "dev",
  cargo: "dev",
  gh: "dev",
  terraform: "dev",
  ssh: "remote",
  aws: "remote",
  gcloud: "remote",
  az: "remote",
  supabase: "remote",
  vercel: "remote",
  program: "fallback",
};

/** A terminal window; the shells and the fallback draw their own symbol inside it. */
const FRAME = "M3 4h18a1.5 1.5 0 0 1 1.5 1.5v13A1.5 1.5 0 0 1 21 20H3a1.5 1.5 0 0 1-1.5-1.5v-13A1.5 1.5 0 0 1 3 4z";
/** A cloud; the cloud tools cut a different mark out of it. */
const CLOUD = "M7 19a5 5 0 0 1-.7-9.95A6 6 0 0 1 17.7 9a4.5 4.5 0 0 1-.2 10z";

export const TOOL_PATHS: Record<ToolId, string> = {
  // A prompt chevron and an underscore.
  powershell: `${FRAME}M5.5 8l4 3.5-4 3.5v-2l1.7-1.5L5.5 10zM11 14.5h6v1.5h-6z`,
  // Three bars, like the "$" of a prompt.
  bash: `${FRAME}M9 8h6v1.5H9zM9 11.25h6v1.5H9zM9 14.5h6V16H9z`,
  // Two diagonal squares, like "%".
  zsh: `${FRAME}M7 8h3.5v3.5H7zM13.5 12.5H17V16h-3.5z`,
  // A chevron and a block cursor.
  cmd: `${FRAME}M5.5 8l4 3.5-4 3.5v-2l1.7-1.5L5.5 10zM11.5 9.5h4v5h-4z`,
  // A diamond in a terminal.
  gitbash: `${FRAME}M12 7.5l4 4.5-4 4.5-4-4.5z`,
  // A triangle in a terminal.
  wsl: `${FRAME}M8.5 8l8 4-8 4z`,
  // A diamond with a node.
  git: "M12 1.5l10.5 10.5L12 22.5 1.5 12zM12 9.2a2.8 2.8 0 1 1 0 5.6 2.8 2.8 0 0 1 0-5.6z",
  // A hull carrying a stack of boxes.
  docker: "M2 14.5h17.5c1.5 0 2.5-1 2.8-2.5 1 .3 1.7.9 2 1.8-.5 3.7-4 7.2-10.8 7.2C6 21 3 18.5 2 14.5zM3 10h3v3H3zM7 10h3v3H7zM11 10h3v3h-3zM7 6h3v3H7zM11 6h3v3h-3zM11 2h3v3h-3z",
  // A hexagon with a smaller one cut out (a wheel's hub).
  kubectl: "M12 2l8.5 4.1v11.8L12 22l-8.5-4.1V6.1zM12 7.5l3.5 1.7v3.6L12 14.5l-3.5-1.7V9.2z",
  // A hexagon with a bar cut out.
  node: "M12 1.5l9 5.2v10.6l-9 5.2-9-5.2V6.7zM10.5 8h3v8h-3z",
  // Two squares meeting at a corner, each with a hole.
  python: "M3 3h9v9H3zM6 6h3v3H6zM12 12h9v9h-9zM15 15h3v3h-3z",
  // A crate: lid and body, with a handle cut out.
  cargo: "M2 3h20v3H2zM3 7h18v13H3zM9 10h6v2H9z",
  // A disc with a triangle cut out.
  gh: "M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zM8 9l4 6 4-6z",
  // Three slanted blocks.
  terraform: "M3 2l6 3.5v7L3 9zM10 5.5l6 3.5v7l-6-3.5zM17 9l5-3v7l-5 3zM10 17l6 3.5V23l-6-3.5z",
  // An arrow each way.
  ssh: "M3 6.5h11V3.5l7 5-7 5v-3H3zM21 17.5H10v3l-7-5 7-5v3h11z",
  // A cloud with a bar.
  aws: `${CLOUD}M8 14.5h8V16H8z`,
  // A cloud with a dot.
  gcloud: `${CLOUD}M12 11.5a2 2 0 1 1 0 4 2 2 0 0 1 0-4z`,
  // A triangle with a smaller one cut out.
  az: "M12 2l10 19H2zM12 9l3.5 7h-7z",
  // A bolt.
  supabase: "M13 2L4 13.5h6.5L10.5 22 20 10h-6.5z",
  // A solid triangle.
  vercel: "M12 3l10 18H2z",
  // A window with two lines of text.
  program: `${FRAME}M5 8h14v1.5H5zM5 11.5h9V13H5z`,
};

export function isToolId(v: string): v is ToolId {
  return (TOOL_IDS as readonly string[]).includes(v);
}

const PROGRAM_TOOLS: Record<string, ToolId> = {
  pwsh: "powershell",
  powershell: "powershell",
  bash: "bash",
  zsh: "zsh",
  cmd: "cmd",
  wsl: "wsl",
  git: "git",
  docker: "docker",
  "docker-compose": "docker",
  kubectl: "kubectl",
  node: "node",
  python: "python",
  python3: "python",
  py: "python",
  cargo: "cargo",
  gh: "gh",
  terraform: "terraform",
  ssh: "ssh",
  aws: "aws",
  gcloud: "gcloud",
  az: "az",
  supabase: "supabase",
  vercel: "vercel",
};

/** The mark for a foreground program (its lowercase name without `.exe`); anything unknown gets the neutral program mark. */
export function toolForProgram(program: string): ToolId {
  return (Object.hasOwn(PROGRAM_TOOLS, program) ? PROGRAM_TOOLS[program] : undefined) ?? "program";
}
