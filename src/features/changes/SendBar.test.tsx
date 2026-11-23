import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SendBar, sendLabel } from "./SendBar";

const draw = (o: { count?: number; via?: "paste" | "clipboard"; asking?: boolean }) =>
  renderToStaticMarkup(<SendBar count={o.count ?? 2} via={o.via ?? "paste"} asking={o.asking ?? false} busy={false} onSend={() => {}} onAsk={() => {}} onDiscard={() => {}} onKeep={() => {}} />);

describe("SendBar", () => {
  it("Send reads Copy for a session with no prompt line", () => {
    expect(sendLabel(1, "paste")).toBe("Send 1 comment");
    expect(sendLabel(3, "paste")).toBe("Send 3 comments");
    expect(sendLabel(3, "clipboard")).toBe("Copy 3 comments");
    expect(draw({ via: "clipboard" })).toContain("Copy 2 comments");
    expect(draw({})).toContain("Send 2 comments");
    expect(draw({})).toContain("Discard all");
  });
  it("Discard all asks first", () => {
    const html = draw({ asking: true });
    expect(html).toContain("Discard 2 comments?");
    expect(html).toContain(">Discard<");
    expect(html).toContain(">Keep<");
    expect(html).not.toContain("Send 2 comments");
  });
});
