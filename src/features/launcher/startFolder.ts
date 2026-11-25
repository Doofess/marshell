import type { DefaultFolder } from "../../generated/DefaultFolder";

export const HOME_FOLDER = "your home folder";

/** The last part of a path, whichever slash it uses. */
export function lastSegment(path: string): string {
  const parts = path.split(/[\\/]+/).filter((p) => p !== "");
  return parts[parts.length - 1] ?? path;
}

/** The name of the folder a session really starts in, for the launcher's "started in" line. */
export function startFolderName(project: string | undefined, profileCwd: string | null | undefined, folder: DefaultFolder | null): string {
  if (project) return project;
  if (profileCwd && profileCwd.trim() !== "") return lastSegment(profileCwd.trim());
  if (folder?.exists && folder.path) return lastSegment(folder.path);
  return HOME_FOLDER;
}

/** Where the launcher's folder dialog opens: the typed folder, else the chosen project, else the default folder when it is there. */
export function browseStart(typed: string | null, project: string | null, folder: DefaultFolder | null): string | null {
  return typed ?? project ?? (folder?.exists && folder.path ? folder.path : null);
}
