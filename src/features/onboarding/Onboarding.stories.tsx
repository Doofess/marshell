import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { ConsentCard } from "./ConsentCard";
import { Detected } from "./Detected";
import { FirstSession } from "./FirstSession";
import { NotificationsStep } from "./NotificationsStep";
import { OnboardingFrame } from "./OnboardingFrame";
import { CONSENT, DETECTIONS } from "./fixtures";

const meta: Meta = { title: "Onboarding", parameters: { layout: "fullscreen" } };
export default meta;

const ALL_ON = { status: true, approve: true, statusline: true };
const box = { maxInlineSize: 640, padding: 16 };

export const DetectedAll: StoryObj = { render: () => <ThemePair label="Screen 1: all three found, each config shown whole, even under a folder with a space" stack><div style={box}><Detected detections={DETECTIONS.allThree} /></div></ThemePair> };
export const DetectedOnlyClaude: StoryObj = { render: () => <ThemePair label="Screen 1: only Claude Code found; the others say Not installed" stack><div style={box}><Detected detections={DETECTIONS.onlyClaude} /></div></ThemePair> };
export const DetectedNone: StoryObj = { render: () => <ThemePair label="Screen 1: nothing found still has a way forward" stack><div style={box}><Detected detections={DETECTIONS.none} /></div></ThemePair> };
export const DetectedProblems: StoryObj = {
  render: () => (
    <ThemePair label="Screen 1: a file that does not parse, one that is read-only, and a managed one, each said in words" stack>
      <div style={box}>
        <Detected detections={[DETECTIONS.unparsable[2]!, DETECTIONS.readOnly[1]!, DETECTIONS.managedPresent[0]!]} />
      </div>
    </ThemePair>
  ),
};
export const ConsentClaude: StoryObj = { render: () => <ThemePair label="Screen 2: one sentence says what changes and where; the backup and the way out are named" stack><div style={box}><ConsentCard scenario={CONSENT.claude} on={ALL_ON} /></div></ThemePair> };
export const ConsentClaudeDiffOpen: StoryObj = { render: () => <ThemePair label="Screen 2: View exact changes shows the unified diff" stack><div style={box}><ConsentCard scenario={CONSENT.claude} on={ALL_ON} open /></div></ThemePair> };
export const ConsentCodexTrust: StoryObj = { render: () => <ThemePair label="Screen 2: Codex adds the step of trusting the hooks in /hooks" stack><div style={box}><ConsentCard scenario={CONSENT.codex} on={ALL_ON} /></div></ThemePair> };
export const ConsentGeminiLimited: StoryObj = { render: () => <ThemePair label="Screen 2: Gemini has no approval hook and no status line to chain, and says so instead of a dead switch" stack><div style={box}><ConsentCard scenario={CONSENT.gemini} on={{ status: true, approve: false, statusline: false }} /></div></ThemePair> };
export const ConsentUnparsable: StoryObj = { render: () => <ThemePair label="Screen 2: a file that does not parse is never edited, and there are no switches" stack><div style={box}><ConsentCard scenario={CONSENT.unparsableGemini} on={ALL_ON} /></div></ThemePair> };
export const ConsentManaged: StoryObj = { render: () => <ThemePair label="Screen 2: a managed file is listed read-only and has no control" stack><div style={box}><ConsentCard scenario={CONSENT.claudeManaged} on={ALL_ON} /></div></ThemePair> };
export const ConsentLongDiff: StoryObj = { render: () => <ThemePair label="Screen 2: a long change folds after 40 lines (a stand-in of 40 entries, and its headline says so)" stack><div style={box}><ConsentCard scenario={CONSENT.codexLong} on={ALL_ON} open /></div></ThemePair> };
export const ConsentReadOnly: StoryObj = { render: () => <ThemePair label="Screen 2: a read-only file is never described as about to change: the card says it is read-only and gives the reason" stack><div style={box}><ConsentCard scenario={CONSENT.readOnlyCodex} on={ALL_ON} /></div></ThemePair> };

const notes = (permission: "not-asked" | "granted" | "denied", os: "windows" | "mac" | "linux") => (
  <div style={box}>
    <NotificationsStep permission={permission} os={os} />
  </div>
);
export const NotificationsNotAsked: StoryObj = { render: () => <ThemePair label="Screen 3: ask once, preview every sound" stack>{notes("not-asked", "windows")}</ThemePair> };
export const NotificationsAllowed: StoryObj = { render: () => <ThemePair label="Screen 3: allowed, and a banner never takes focus" stack>{notes("granted", "windows")}</ThemePair> };
export const NotificationsBlocked: StoryObj = { render: () => <ThemePair label="Screen 3: blocked by the OS, with where to turn it on and what still works" stack>{notes("denied", "windows")}</ThemePair> };
export const NotificationsMac: StoryObj = { render: () => <ThemePair label="Screen 3 on macOS (design only: Windows 11 is the only tested platform)" stack>{notes("not-asked", "mac")}</ThemePair> };
export const NotificationsLinux: StoryObj = { render: () => <ThemePair label="Screen 3 on Linux (design only)" stack>{notes("denied", "linux")}</ThemePair> };
export const FirstSessionScreen: StoryObj = { render: () => <ThemePair label="Screen 4: the launcher, pre-filled with the most recent git folder; Enter starts it" stack><div style={box}><FirstSession /></div></ThemePair> };

export const FrameStep1: StoryObj = { render: () => <ThemePair label="The frame around screen 1: one primary button, no Back" stack><OnboardingFrame step="detected" title="What Marshell found" primary="Continue"><Detected detections={DETECTIONS.allThree} /></OnboardingFrame></ThemePair> };
export const FrameStep2: StoryObj = { render: () => <ThemePair label="The frame around screen 2: Back appears; one card per CLI" stack><OnboardingFrame step="consent" title="What Marshell may change" primary="Allow these changes" secondary="Skip all: use limited status"><ConsentCard scenario={CONSENT.claude} on={ALL_ON} /></OnboardingFrame></ThemePair> };
export const FrameStep3: StoryObj = { render: () => <ThemePair label="The frame around screen 3" stack><OnboardingFrame step="notifications" title="Notifications" primary="Continue"><NotificationsStep permission="not-asked" os="windows" /></OnboardingFrame></ThemePair> };
export const FrameStep4: StoryObj = { render: () => <ThemePair label="The frame around screen 4: the last step" stack><OnboardingFrame step="first-session" title="Start your first session" primary="Start"><FirstSession /></OnboardingFrame></ThemePair> };
export const FrameStep2SmallWindow: StoryObj = {
  render: () => (
    <ThemePair label="Screen 2 in a 720 by 480 window with three consent cards open: the cards scroll in the body; Skip and Allow these changes stay in view" stack>
      <div style={{ inlineSize: 720, blockSize: 480 }}>
        <OnboardingFrame step="consent" title="What Marshell may change" primary="Allow these changes" secondary="Skip all: use limited status">
          <ConsentCard scenario={CONSENT.claude} on={ALL_ON} open />
          <ConsentCard scenario={CONSENT.codex} on={ALL_ON} open />
          <ConsentCard scenario={CONSENT.gemini} on={{ status: true, approve: false, statusline: false }} open />
        </OnboardingFrame>
      </div>
    </ThemePair>
  ),
};
