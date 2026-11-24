import { describe, expect, it } from "vitest";
import { threadCopy as c } from "./threadCopy";

describe("threadCopy", () => {
  it("agrees in number", () => {
    expect(c.toResolve(1)).toBe("1 comment to resolve");
    expect(c.toResolve(3)).toBe("3 comments to resolve");
    expect(c.toResolve(1200)).toBe("1,200 comments to resolve");
    expect(c.talk(1)).toBe("1 review or comment");
    expect(c.talk(3)).toBe("3 reviews and comments");
    expect(c.addAll(1)).toBe("Add the unresolved comment to send");
    expect(c.addAll(3)).toBe("Add all 3 unresolved");
    expect(c.added(1, 4)).toBe("Added 1 comment from GitHub. 4 to send.");
    expect(c.added(3, 4)).toBe("Added 3 comments from GitHub. 4 to send.");
    expect(c.leftOver(1)).toBe("1 comment from GitHub didn't fit in the send and stays picked.");
    expect(c.leftOver(3)).toBe("3 comments from GitHub didn't fit in the send and stay picked.");
    expect(c.newComments(1, 7)).toBe("1 new comment on #7.");
    expect(c.newComments(3, 7)).toBe("3 new comments on #7.");
    expect(c.minimized(1)).toBe("1 comment is hidden on GitHub.");
    expect(c.minimized(3)).toBe("3 comments are hidden on GitHub.");
    expect(c.addSome(2, 1)).toBe("Add 2 unresolved · 1 already sent");
  });
  it("names people and agents", () => {
    expect(c.bot("dependabot")).toBe("dependabot (bot)");
    expect(c.sentTo("Claude Code")).toBe("Sent to Claude Code");
    expect(c.sentTo(null)).toBe("Copied");
    expect(c.yourNote("hi")).toBe("Your note: hi");
    expect(c.reviewBy("alice", "changes_requested")).toBe("Review by alice · Changes requested");
    expect(c.reviewBy("alice", "approved")).toBe("Review by alice · Approved");
    expect(c.reviewBy("alice", "dismissed")).toBe("Review by alice · Dismissed");
    expect(c.reviewBy("alice", "commented")).toBe("Review by alice");
    expect(c.reviewBy("alice", null)).toBe("Review by alice");
    expect(c.commentBy("alice")).toBe("Comment by alice");
    expect(c.replyBy("alice")).toBe("Reply by alice");
    expect(c.replyLabel("alice")).toBe("Reply to alice on GitHub");
    expect(c.noteLabel("alice")).toBe("Your note on alice's comment");
    expect(c.picked(2)).toBe("Added to send. 2 to send.");
    expect(c.replied(5)).toBe("Replied on #5.");
    expect(c.moreThreads(2)).toBe("And 2 more comment threads on GitHub.");
    expect(c.moreComments(2)).toBe("And 2 more on GitHub.");
    expect(c.readFailed(5, "offline")).toBe("Can't read the comments on #5: offline");
    expect(c.noteAdded(2)).toBe("Note added. 2 to send.");
    expect(c.menu.editNote).toBe("Edit your note…");
    expect(c.replyHint).toBe("Ctrl+Enter replies on GitHub · Enter starts a new line · Esc closes");
  });
});
