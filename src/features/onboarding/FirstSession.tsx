import { nextSessionNumber } from "../sidebar/sessionNumber";
import { DEFAULT_CHOICE, PROJECTS, TAKEN_NUMBERS } from "../launcher/fixtures";
import { Launcher } from "../launcher/Launcher";
import "./Onboarding.css";

/** Screen 4: the launcher, pre-filled with the most recent git folder and shown inline (not a modal), so Enter starts the first session. */
export function FirstSession() {
  return (
    <div className="first-session">
      <p className="first-session__hint">Enter starts your first session.</p>
      <div className="first-session__frame">
        <Launcher view={{ choice: DEFAULT_CHOICE, projects: PROJECTS, query: null, activeIndex: 0, nextNumber: nextSessionNumber(TAKEN_NUMBERS) }} />
      </div>
    </div>
  );
}
