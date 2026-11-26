// Whether winget-submit sends a release to winget-pkgs (PLAN §9, slice 9c decisions 1 and 4).
// Usage, in the workflow: node scripts/release/winget-hold.mjs, with EVENT, TAG, HAS_TOKEN and WINGET_HOLD
// in the environment. It appends submit=true or submit=false to GITHUB_OUTPUT and prints a notice when the
// release isn't submitted.
import { appendFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// Only an unset (empty) variable or exactly "false" lets a tag submit, so a typo holds rather than submits.
export function isHeld(value) {
  return value !== undefined && value !== "" && value !== "false";
}

/** @param {{ event: string, tag: string, hasToken: string, hold?: string }} run */
export function submitDecision({ event, tag, hasToken, hold }) {
  if (hasToken !== "true") {
    return {
      submit: false,
      notice: `No WINGET_TOKEN secret, so ${tag} wasn't submitted to winget-pkgs. Add the secret (docs/RELEASING.md), then run this workflow with action 'submit' and tag '${tag}'.`,
    };
  }
  // A manual submit is a request for this one release, so the hold doesn't apply to it.
  if (event !== "workflow_dispatch" && isHeld(hold)) {
    return {
      submit: false,
      notice: `The repository variable WINGET_HOLD is set, so ${tag} wasn't submitted to winget-pkgs. To submit it, run this workflow with action 'submit' and tag '${tag}'. To let tags submit again, delete the variable (docs/RELEASING.md).`,
    };
  }
  return { submit: true };
}

function main() {
  const { GITHUB_OUTPUT, TAG, EVENT, HAS_TOKEN, WINGET_HOLD } = process.env;
  if (!GITHUB_OUTPUT || !TAG) {
    throw new Error("winget-hold.mjs needs GITHUB_OUTPUT and TAG in its environment.");
  }
  const decision = submitDecision({ event: EVENT ?? "", tag: TAG, hasToken: HAS_TOKEN ?? "", hold: WINGET_HOLD });
  appendFileSync(GITHUB_OUTPUT, `submit=${decision.submit}\n`);
  if (decision.notice) console.log(`::notice::${decision.notice}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
