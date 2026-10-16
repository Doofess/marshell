import { type Dispatch, useEffect, useReducer, useRef, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { ViewLayout } from "../../generated/ViewLayout";
import { layout } from "../../lib/api/layout";
import { DEFAULT_VIEW, type ViewAction, type ViewState, toSaved, viewReducer } from "./viewState";

const SAVE_DELAY_MS = 300;

type Internal = ViewAction | { type: "loaded"; view: ViewLayout };
const reduce = (s: ViewState, a: Internal): ViewState => (a.type === "loaded" ? a.view : viewReducer(s, a));

/**
 * The window layout, loaded from the core once and saved back 300 ms after it changes.
 * `openIds` is the open tabs, or undefined until the session list has arrived (so a list that is still empty
 * is never mistaken for "every tab closed"). The third value is the whole document, or null until the read answers
 * (and for good if it failed).
 *
 * A failed read starts from the defaults and settles the same way a good one does, so the window still opens.
 * Saving waits for the read to settle and then only writes what differs from what was loaded, so opening the window
 * never writes the defaults over the user's file.
 */
export function useViewState(endpoint: Endpoint | null, openIds: string[] | undefined): [ViewState, Dispatch<ViewAction>, LayoutDoc | null] {
  const [state, dispatch] = useReducer(reduce, DEFAULT_VIEW);
  const [doc, setDoc] = useState<LayoutDoc | null>(null);
  const [settled, setSettled] = useState(false);
  /** The last view the core is known to hold, as JSON. */
  const saved = useRef(JSON.stringify(DEFAULT_VIEW));

  useEffect(() => {
    if (!endpoint) return;
    let live = true;
    layout
      .get(endpoint)
      .then((d) => {
        if (!live) return;
        saved.current = JSON.stringify(d.view);
        setDoc(d);
        dispatch({ type: "loaded", view: d.view });
      })
      .catch(() => {
        if (live) saved.current = JSON.stringify(DEFAULT_VIEW);
      })
      .finally(() => live && setSettled(true));
    return () => {
      live = false;
      setSettled(false);
    };
  }, [endpoint]);

  const openKey = openIds?.join("\n");
  useEffect(() => {
    if (settled && openIds !== undefined) dispatch({ type: "sessions", openIds });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `openKey` stands for the contents of `openIds`, a new array on every render.
  }, [settled, openKey]);

  useEffect(() => {
    if (!endpoint || !settled) return;
    const view = toSaved(state);
    const json = JSON.stringify(view);
    if (json === saved.current) return;
    const timer = setTimeout(() => {
      layout
        .putView(endpoint, view)
        .then(() => {
          saved.current = json;
        })
        .catch(() => {}); // Not saved: the next change tries again.
    }, SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [endpoint, settled, state]);

  return [state, dispatch as Dispatch<ViewAction>, doc];
}
