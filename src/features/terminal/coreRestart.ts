/** What a session that ended (its process exited, or the core restarted) tells the user to do. */
export const SESSION_ENDED = "Session ended. Close it with Ctrl+Shift+W.";

/** Written into the terminal when the core restarted: the old session is gone. */
export const ENDED_LINE = `\r\n\x1b[2m${SESSION_ENDED}\x1b[0m\r\n`;

/** Written into the terminal when the core gave up after too many restarts. */
export const FAILED_LINE = "\r\n\x1b[2mMarshell's core stopped and could not restart. Restart Marshell to continue.\x1b[0m\r\n";
