import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api/client";
import { refusable } from "../../lib/api/sessions";
import { copyResumeCommand, openInSystemTerminal } from "./resumeActions";

const PREFIX = "Couldn't open a system terminal: ";

describe("openInSystemTerminal", () => {
  it("says the prefix once for an error the core answered with the OS text alone", async () => {
    const announce = vi.fn();
    // The route as the app calls it: `refusable` rethrows what the client throws for the core's 500, whose message is
    // the response text.
    const failing = () => refusable(() => Promise.reject(new ApiError(500, "The system cannot find the file specified. (os error 2)")));
    await openInSystemTerminal(failing, announce);
    expect(announce).toHaveBeenCalledTimes(1);
    const text = announce.mock.calls[0]?.[0] as string;
    expect(text).toBe(`${PREFIX}The system cannot find the file specified. (os error 2)`);
    expect(text.split(PREFIX).length - 1).toBe(1);
  });

  it("gives the core's reason as it is when the core refuses", async () => {
    const announce = vi.fn();
    const body = JSON.stringify({ reason: "It's open in a system terminal." });
    await openInSystemTerminal(() => refusable(() => Promise.reject(new ApiError(409, body))), announce);
    expect(announce).toHaveBeenCalledWith("It's open in a system terminal.");
  });

  it("confirms an opened console", async () => {
    const announce = vi.fn();
    await openInSystemTerminal(() => Promise.resolve({ ok: true }), announce);
    expect(announce).toHaveBeenCalledWith("Opened in a system terminal.");
  });
});

describe("copyResumeCommand", () => {
  it("prefixes a failed copy once", async () => {
    const announce = vi.fn();
    await copyResumeCommand(() => Promise.reject(new ApiError(500, "disk on fire")), vi.fn(), announce);
    expect(announce).toHaveBeenCalledWith("Couldn't copy the resume command: disk on fire");
  });
});
