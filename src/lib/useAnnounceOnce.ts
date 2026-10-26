import { useEffect, useEffectEvent } from "react";

/**
 * Says `text` through the page's announcer once each time it appears (null: nothing to say). A section that shows an error as
 * plain text uses this instead of drawing a live region of its own. `announce` is read through an effect event, so a parent that
 * re-renders with a new callback never says the same message again.
 */
export function useAnnounceOnce(text: string | null | undefined, announce?: (text: string) => void) {
  const say = useEffectEvent((t: string) => announce?.(t));
  useEffect(() => {
    if (text) say(text);
  }, [text]);
}
