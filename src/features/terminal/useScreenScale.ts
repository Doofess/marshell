import { useEffect, useState } from "react";
import { screenScale } from "./columns";

/** The screen's scale, kept current when the window moves to a screen with another one or the system's scale changes. */
export function useScreenScale(): number {
  const [scale, setScale] = useState(screenScale);
  useEffect(() => {
    // The query matches only the current scale, so it changes exactly when the scale does.
    const query = window.matchMedia(`(resolution: ${scale}dppx)`);
    const update = () => setScale(screenScale());
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, [scale]);
  return scale;
}
