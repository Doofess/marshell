import type { ReactElement } from "react";

/** A synchronous hook host for node-only tests. Flush after events/timers to render
 * state updates and run effects; unmount runs every effect's cleanup. */
let active: Host | undefined;
type Slot = { value?: any; deps?: readonly unknown[]; cleanup?: void | (() => void) };
type Host = { slots: Slot[]; cursor: number; dirty: boolean; effects: (() => void)[] };
const same = (a?: readonly unknown[], b?: readonly unknown[]) => !!a && !!b && a.length === b.length && a.every((v, i) => Object.is(v, b[i]));
function slot(): [Host, Slot] {
  if (!active) throw new Error("Render hooks inside mountHook");
  const host = active;
  const i = host.cursor++;
  return [host, host.slots[i] ??= {}];
}
export const hooks = {
  useState(initial: any) {
    const [host, s] = slot();
    if (!("value" in s)) s.value = typeof initial === "function" ? initial() : initial;
    return [s.value, (next: any) => {
      const value = typeof next === "function" ? next(s.value) : next;
      if (!Object.is(value, s.value)) { s.value = value; host.dirty = true; }
    }];
  },
  useReducer(reducer: (state: any, action: any) => any, initial: any) {
    const [value, set] = hooks.useState(initial);
    return [value, (action: any) => set((state: any) => reducer(state, action))];
  },
  useRef(initial: any) {
    const [, s] = slot();
    return s.value ??= { current: initial };
  },
  useMemo(factory: () => any, deps?: readonly unknown[]) {
    const [, s] = slot();
    if (!same(s.deps, deps)) { s.value = factory(); s.deps = deps; }
    return s.value;
  },
  useCallback(fn: (...args: any[]) => any, deps?: readonly unknown[]) {
    return hooks.useMemo(() => fn, deps);
  },
  useEffect(effect: () => void | (() => void), deps?: readonly unknown[]) {
    const [host, s] = slot();
    if (same(s.deps, deps)) return;
    s.deps = deps;
    host.effects.push(() => { s.cleanup?.(); s.cleanup = effect(); });
  },
  useId() { return hooks.useMemo(() => "test-id", []); },
};
export function mountHook<T>(render: () => T) {
  const host: Host = { slots: [], cursor: 0, dirty: true, effects: [] };
  let result: T;
  const flush = () => {
    let attempts = 0;
    while (host.dirty) {
      if (++attempts > 100) throw new Error("Hook host did not settle");
      host.dirty = false;
      host.cursor = 0;
      active = host;
      try { result = render(); } finally { active = undefined; }
      for (const effect of host.effects.splice(0)) effect();
    }
    return result!;
  };
  flush();
  return {
    get current() { return result!; },
    flush,
    rerender() { host.dirty = true; return flush(); },
    unmount() { for (const s of host.slots) s.cleanup?.(); },
  };
}

/** Inspect JSX without a browser; event callbacks are the component's real handlers. */
export function elements(node: any): ReactElement<any>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!node || typeof node !== "object" || !("props" in node)) return [];
  return [node, ...elements(node.props.children)];
}
