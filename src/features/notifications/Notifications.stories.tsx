import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { batchBanner, bannerFor, escalationBanner, type NotifyEvent, type Os } from "./banner";
import { OsBanner } from "./OsBanner";

const meta: Meta = { title: "Notifications", parameters: { layout: "fullscreen" } };
export default meta;

const perm: NotifyEvent = { kind: "needs-permission", number: 3, name: "api-server", project: "my-app", detail: "npm test" };
const question: NotifyEvent = { kind: "needs-question", number: 2, name: "infra", project: "terraform" };
const billing: NotifyEvent = { kind: "needs-permission", number: 1, name: "billing", project: "payments", detail: "npm test" };

const show = (os: Os, e: NotifyEvent) => <OsBanner os={os} banner={bannerFor(e, os)!} caption />;

export const WindowsPermission: StoryObj = { render: () => <ThemePair label="Windows: a request names the session and the command and offers Allow and Deny; a click only focuses the session">{show("windows", perm)}</ThemePair> };
export const WindowsRisky: StoryObj = { render: () => <ThemePair label="A risky request shows only Review and never the command">{show("windows", { ...perm, risky: true, detail: "rm -rf node_modules" })}</ThemePair> };
export const WindowsQuestion: StoryObj = { render: () => <ThemePair label="A question says so and opens the session">{show("windows", question)}</ThemePair> };
export const WindowsError: StoryObj = { render: () => <ThemePair label="An error says why and when it resets">{show("windows", { kind: "error", number: 6, name: "scratch", project: "playground", detail: "rate limit reached", resetsAt: "3:40 pm" })}</ThemePair> };
export const WindowsDone: StoryObj = { render: () => <ThemePair label="Done, only for a session that has been in the background for more than 30 seconds">{show("windows", { kind: "done", number: 5, name: "docs", project: "marketing", detail: "Edited 4 files", backgroundMs: 95_000 })}</ThemePair> };
export const WindowsBatched: StoryObj = { render: () => <ThemePair label="Three requests within 3 seconds fold into one banner">{<OsBanner os="windows" banner={batchBanner([perm, question, billing], "windows")!} caption />}</ThemePair> };
export const WindowsEscalation: StoryObj = { render: () => <ThemePair label="Escalation: once, after a wait, with the wait in minutes">{<OsBanner os="windows" banner={escalationBanner({ ...perm, waitedMs: 60_000 }, "windows")} caption />}</ThemePair> };
export const MacPermission: StoryObj = { render: () => <ThemePair label="macOS (design only): the same content with the platform's rounder corners">{show("mac", perm)}</ThemePair> };
export const LinuxPermission: StoryObj = { render: () => <ThemePair label="Linux (design only): only Open, since Allow and Deny need the desktop to offer actions">{show("linux", perm)}</ThemePair> };

const QUIET: Array<{ what: string; why: string; event: NotifyEvent }> = [
  { what: "A request while Marshell is the focused window", why: "You are already looking at it.", event: { ...perm, windowFocused: true } },
  { what: "A session that looks stuck", why: "It may only be thinking; the sidebar shows it, nothing interrupts you.", event: { kind: "stuck", number: 7, name: "worker" } },
  { what: "A subagent finished", why: "Too frequent to be worth a banner.", event: { kind: "subagent-done", number: 3, name: "api-server" } },
  { what: "Done, after 20 seconds in the background", why: "Under 30 seconds you have probably seen it.", event: { kind: "done", number: 5, name: "docs", backgroundMs: 20_000 } },
];
export const QuietCases: StoryObj = {
  render: () => (
    <ThemePair label="When no banner appears, and why" stack>
      <table style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th scope="col">Event</th>
            <th scope="col">Banner</th>
            <th scope="col">Why</th>
          </tr>
        </thead>
        <tbody>
          {QUIET.map((q) => (
            <tr key={q.what}>
              <td>{q.what}</td>
              <td>{bannerFor(q.event, "windows") === null ? "None" : "Shown"}</td>
              <td>{q.why}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </ThemePair>
  ),
};
export const Rtl: StoryObj = { render: () => <ThemePair label="A Hebrew session name reads in its own direction, with the buttons in order">{show("windows", { ...perm, name: "שרת-הבדיקות", project: "האפליקציה שלי" })}</ThemePair> };
