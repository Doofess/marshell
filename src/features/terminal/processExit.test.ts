import { describe, expect, it } from "vitest";
import { processExited } from "./processExit";

describe("processExited", () => {
  it("writes the line, then tells the owner the process ended", () => {
    const log: string[] = [];
    const write = (text: string, done: () => void) => {
      log.push(`write ${text.includes("Process exited with code 3.") ? "line" : text}`);
      done();
    };
    processExited(write, 3, (code) => log.push(`exited ${code}`));
    expect(log).toEqual(["write line", "exited 3"]);
  });
  it("with no owner listening it only writes the line", () => {
    const log: string[] = [];
    processExited((text, done) => (log.push(text), done()), 0, undefined);
    expect(log).toHaveLength(1);
  });
});
