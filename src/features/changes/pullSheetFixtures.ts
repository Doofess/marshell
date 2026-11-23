import type { PushPlan } from "../../generated/PushPlan";

/** A push plan for the sheets' tests: two commits to push on `feat`, based on `main`. */
export const plan = (o: Partial<PushPlan> = {}): PushPlan => ({
  checkout: "C:/work/widgets",
  branch: "feat",
  head: "a".repeat(40),
  moved: false,
  on_github: false,
  to_push: 2,
  commits: [{ id: "aaaaaaa", subject: "Add a thing" }],
  more: 0,
  uncommitted: 0,
  base: "main",
  title: "Add a thing",
  body: "",
  refusal: null,
  ...o,
});
