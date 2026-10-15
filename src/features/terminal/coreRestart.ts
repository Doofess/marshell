/** Written into the terminal when the core restarted: the old session is gone, Enter starts a new one. */
export const ENDED_LINE = "\r\n\x1b[2mMarshell's core restarted, so this session ended. Press Enter for a new shell.\x1b[0m\r\n";

/** Written into the terminal when the core gave up after too many restarts. */
export const FAILED_LINE = "\r\n\x1b[2mMarshell's core stopped and could not restart. Restart Marshell to continue.\x1b[0m\r\n";
