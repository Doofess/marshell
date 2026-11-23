import { useEffect, useRef, useState } from "react";
import { cancelPush, createPull, failureOf, readPlan, readPush, startPush } from "./githubApi";
import { githubCopy as gc } from "./githubCopy";
import { INITIAL_RUN, PUSH_POLL_MS, type RunEvent, type RunState, pushStep } from "./pushRun";

export interface CreateFields {
  title: string;
  body: string;
  base: string;
  draft: boolean;
}

export interface PushRunOptions {
  tab: string;
  /** True for the Create pull request sheet: after the push (or with nothing to push) it creates the pull request. */
  create: boolean;
  announce(text: string): void;
  /** The flow is over and succeeded: the owner closes the sheet and asks GitHub again. */
  onDone(): void;
  /** Where the sheet starts; a state other than `loading` skips the plan call. */
  initial?: RunState;
  /** The Create sheet's fields as they are now. */
  fields?: () => CreateFields;
}

/**
 * The effects of a push sheet: the plan call on open, the push, the poll every `PUSH_POLL_MS`, the cancel and the create.
 * `pushStep` holds the flow; this sends it events. Unmounting stops the polling and never cancels the push.
 */
export function usePushRun(o: PushRunOptions) {
  const ref = useRef<RunState>(o.initial ?? INITIAL_RUN);
  const [state, setState] = useState<RunState>(ref.current);
  const live = useRef(o);
  const alive = useRef(true);
  useEffect(() => {
    live.current = o;
  });
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const send = (e: RunEvent) => {
    const prev = ref.current;
    const next = pushStep(prev, e);
    if (next === prev) return;
    ref.current = next;
    if (alive.current) setState(next);
    after(prev, next);
  };

  const after = (prev: RunState, next: RunState) => {
    const { announce, onDone, create } = live.current;
    if (next.kind === "running" && next.id === null && prev.kind !== "running") return begin();
    if (prev.kind === "running") {
      if (next.kind === "pushed") {
        if (create) {
          send({ type: "create_started" });
          return makePull();
        }
        announce(gc.pushed(next.plan?.to_push ?? null) + (next.reason ? ` ${next.reason}` : ""));
        return onDone();
      }
      if (next.kind === "failed") return announce(next.reason ?? "");
      if (next.kind === "cancelled") return announce(gc.stopped);
    }
    if (next.kind === "creating" && (prev.kind === "confirm" || prev.kind === "create_failed")) makePull();
  };

  const begin = () => {
    const plan = ref.current.plan;
    if (plan === null) return;
    startPush(live.current.tab, { checkout: plan.checkout, branch: plan.branch, head: plan.head }).then(
      (r) => send({ type: "started", id: r.id }),
      (e) => send({ type: "start_refused", reason: failureOf(e).reason }),
    );
  };

  const makePull = () => {
    const { tab, fields, announce, onDone } = live.current;
    const plan = ref.current.plan;
    const f = fields?.();
    if (plan === null || !f) return send({ type: "create_failed", reason: gc.titleLength });
    createPull(tab, { checkout: plan.checkout, branch: plan.branch, head: plan.head, base: f.base.trim(), title: f.title.trim(), body: f.body, draft: f.draft }).then(
      (r) => {
        send({ type: "created", number: r.number, url: r.url });
        onDone();
      },
      (e) => {
        const reason = failureOf(e).reason;
        send({ type: "create_failed", reason });
        announce(reason);
      },
    );
  };

  useEffect(() => {
    if (ref.current.kind !== "loading") return;
    readPlan(o.tab, o.create).then(
      (plan) => alive.current && send({ type: "plan", plan }),
      (e) => alive.current && send({ type: "plan_failed", reason: failureOf(e).reason }),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once, on open
  }, []);

  useEffect(() => {
    if (state.kind !== "running" || state.id === null) return;
    const id = state.id;
    let stop = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let fails = 0;
    const tick = async () => {
      try {
        const progress = await readPush(id, ref.current.seen);
        if (stop) return;
        fails = 0;
        send({ type: "progress", progress });
        if (progress.state !== "running") return;
      } catch (e) {
        if (stop) return;
        if (++fails >= 5) return send({ type: "progress", progress: { state: "failed", start: ref.current.seen, lines: [], total: ref.current.seen, reason: failureOf(e).reason } });
      }
      timer = setTimeout(tick, PUSH_POLL_MS);
    };
    void tick();
    return () => {
      stop = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- one loop per running push
  }, [state.kind, state.id]);

  return {
    state,
    confirm: () => send({ type: "confirm", create: live.current.create }),
    tryAgain: () => send({ type: "try_again" }),
    cancel: () => {
      const id = ref.current.id;
      if (id !== null) cancelPush(id).catch((e) => live.current.announce(failureOf(e).reason));
    },
  };
}
