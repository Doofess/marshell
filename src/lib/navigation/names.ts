const TOP_NAMES: Record<string, string> = { palette: "Command palette", sidebar: "Sidebar", launcher: "New session", "panel-browser": "Browser", "panel-files": "Files" };

/** What the hint and the announcements name for a top-bar control. An unknown id is returned as it is. */
export const topName = (id: string): string => TOP_NAMES[id] ?? id;
