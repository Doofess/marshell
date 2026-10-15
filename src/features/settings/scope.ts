export type Scope = "user" | "project" | "local" | "managed" | "marshell";
export const SCOPES: Scope[] = ["user", "project", "local", "managed", "marshell"];
export const SCOPE_LABEL: Record<Scope, string> = { user: "User", project: "Project", local: "Local", managed: "Managed", marshell: "Marshell" };
export const SCOPE_NOTE: Record<Scope, string> = {
  user: "Applies to every project on this computer.",
  project: "Applies to this project and is shared through git.",
  local: "Applies to this project on this computer only.",
  managed: "Set by your organisation. Marshell can't change it.",
  marshell: "Marshell's own setting, kept in ~/.marshell/settings.json.",
};
/** A shape as well as a word, so a scope never relies on colour. */
export const SCOPE_SHAPE: Record<Scope, "circle" | "square" | "diamond" | "lock" | "flag"> = { user: "circle", project: "square", local: "diamond", managed: "lock", marshell: "flag" };
export const isReadOnlyScope = (s: Scope) => s === "managed";
