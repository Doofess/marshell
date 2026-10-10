import { isTool } from "../../components/AgentMark/kinds";
import type { RowModel } from "./types";

/**
 * Whether a session gets Marshell's prompt line. An AI agent does; a manual CLI is a plain terminal, so the terminal
 * itself is where you type and a second input would only be a place to type into the wrong thing.
 */
export const hasPromptLine = (row: RowModel | null): boolean => row !== null && !isTool(row.agent);
