import type { Detection } from "../onboarding/types";
import { launcherSetup, shownNumber } from "./launcherSetup";
import { useEffect, useMemo, useRef, useState } from "react";
import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { RepoScripts } from "../../generated/RepoScripts";
import type { Task } from "../../generated/Task";
import { allowScripts, getScripts, runScript } from "../../lib/api/scripts";
import { afterCreate } from "../scripts/afterSetup";
import { confirmScripts } from "../scripts/confirmScripts";
import type { Endpoint } from "../../generated/Endpoint";
import type { FolderInfo } from "../../generated/FolderInfo";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { ShellAgents } from "../../generated/ShellAgents";
import { catalogApi } from "../../lib/api/catalog";
import { ApiError } from "../../lib/api/client";
import { folders as foldersApi } from "../../lib/api/folders";
import { layout as layoutApi } from "../../lib/api/layout";
import { profiles as profilesApi } from "../../lib/api/profiles";
import { projects as projectsApi } from "../../lib/api/projects";
import { createSession } from "../../lib/api/elevated";
import { sessions as sessionsApi } from "../../lib/api/sessions";
import { tasksApi } from "../../lib/api/tasks";
import { pickFolder } from "../../lib/folderPicker";
import { useAppearance } from "../appearance/AppearanceProvider";
import { withColorScheme } from "../appearance/terminalSettings";
import type { Project } from "../launcher/filter";
import { Launcher, type TaskConfirm } from "../launcher/Launcher";
import { typedPath, type PathCheck } from "../launcher/folderRows";
import { openFolder, openList, type OpenLists } from "../launcher/launcherKeys";
import { effectiveAgent, offerFor } from "../launcher/shellAgents";
import { offeredMode } from "../modes/sessionModes";
import { taskNameFor } from "../tasks/taskName";
import { appScheme } from "../terminal/terminalTheme";
import { adminChip, programById, startingText, WAITING_FOR_ADMIN, type LauncherChoice } from "../launcher/launchChoice";
import type { Os } from "../../lib/keymap";
import { browseFolder, checkState, chooseFolder, latestOnly, createRequest, createThenLaunch, initialFocus, launchRequest, progressText, refocus, rememberTasks, replansAfter, type FocusState, liveAgents, liveShells, nameSource, preselect, preselectMode, startFailure, startProblem, taskRequest, toLauncherProject, wantsTask } from "./launchPrograms";

// The worktree flag is part of this state and nothing else: it is never read from or written to the remembered choices, so the chip starts off every time.
// The shell is empty until the profiles load: then the default one (or the remembered one) is shown. An agent the shell lacks becomes Just the shell.
const INITIAL: LauncherChoice = { shell: "", agent: "claude", projectId: "", mode: "manual", pace: "Balanced", worktree: false };

/** `layout` is read on every opening, so a folder used a moment ago is already remembered; null when it could not be read. */
type Loaded = { projects: Project[]; doc: ProfilesDoc; layout: LayoutDoc | null; catalog: AgentCatalog | null };

type Props = {
  detections?: Detection[];
  detectionFailed?: ReadonlySet<string>;
  endpoint: Endpoint | null;
  /** The palette's "New session as administrator" opens the launcher with the Administrator chip on. */
  initialElevated?: boolean;
  /** The platform; Administrator is available on Windows only. */
  os?: Os;
  /** The number the new session takes. */
  nextNumber: number;
  /** The last measured terminal size, read when Start is pressed. */
  size: () => { cols: number; rows: number };
  /** The core created the session. `text` is what its tab says until the first output. */
  onStarted: (created: CreateSessionResponse, text: string) => void;
  /** A task's setup tab was started: the agent's request waits in the window until that tab ends. Called before `onStarted` for the setup tab. */
  onSetupStarted: (setup: { tabId: string; request: CreateSessionRequest; text: string }) => void;
  /** Says something through the window's announcer. */
  announce: (text: string) => void;
  onClose: () => void;
};

/** A task already made whose own marshell.json is waiting for a yes before its setup runs. */
type Held = { task: Task; request: CreateSessionRequest; text: string; scripts: RepoScripts };

/**
 * The launcher on the core: it lists the recent folders, the agents and the shell profiles, and Start creates the session.
 * Mounted only while open, so every opening starts fresh. When the core refuses, its text shows under Start and the launcher stays.
 */
export function LiveLauncher({ detections, detectionFailed, endpoint, initialElevated = false, os = "windows", nextNumber, size, onStarted, onSetupStarted, announce, onClose }: Props) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const { appearance } = useAppearance();
  const [choice, setChoice] = useState(INITIAL);
  // Launcher state only: not in the choice, not remembered, gone when the launcher closes.
  const [bypassAck, setBypassAck] = useState<string | null>(null);
  const [elevatedWanted, setElevatedWanted] = useState(initialElevated);
  // Once the user picks a shell or an agent here, the remembered one no longer applies.
  const [shellPicked, setShellPicked] = useState(false);
  const [agentPicked, setAgentPicked] = useState(false);
  const [modePicked, setModePicked] = useState(false);
  // One state for the three lists, so opening one closes the others.
  const [lists, setLists] = useState<OpenLists>({ query: null, shellsOpen: false, agentsOpen: false });
  const { query, shellsOpen, agentsOpen } = lists;
  // What each shell said it can start, asked once per shell per opening and kept under the profile id the answer names. "failed" is under the id asked.
  const [answers, setAnswers] = useState<Record<string, ShellAgents | "failed">>({});
  const asked = useRef(new Set<string>());
  const [activeIndex, setActiveIndex] = useState(0);
  const [prompt, setPrompt] = useState("");
  const [error, setError] = useState<string>();
  // Why the lists did not load. Unlike `error` no edit clears it: Start stays disabled until they do.
  const [loadError, setLoadError] = useState<string>();
  const [starting, setStarting] = useState(false);
  // The number the pending start reserved; the live list moves on once the core has the row.
  const frozen = useRef<number | undefined>(undefined);
  // The task name once the user has typed in the field; until then it follows the prompt.
  const [nameEdit, setNameEdit] = useState<string | null>(null);
  // The confirm view of a task, or null while the launcher shows its sentence.
  const [confirm, setConfirm] = useState<TaskConfirm | null>(null);
  // Each plan request gets a number, so a late answer to a plan the user has left is dropped.
  const planSeq = useRef(0);
  // A task already made for this request: if the session then fails to start, trying again must not make a second worktree. Back forgets it.
  const tasks = useMemo(rememberTasks, []);
  // What the confirm view says while it works: the worktree being made, then the program starting in it.
  const [phase, setPhase] = useState<"creating" | "starting">("creating");
  const inFlight = useRef(false);
  // Set while the confirm view shows the worktree's own marshell.json for a yes; the task is already made.
  const held = useRef<Held | null>(null);
  const root = useRef<HTMLDivElement>(null);

  // Keys go to the launcher, not the terminal behind it; closing hands focus back if that terminal is still there.
  useEffect(() => {
    const before = document.activeElement;
    if (root.current) initialFocus(root.current)?.focus();
    return () => {
      if (before instanceof HTMLElement && before !== document.body && before.isConnected) before.focus();
    };
  }, []);

  useEffect(() => {
    if (!endpoint) return;
    let current = true;
    setLoaded(null);
    setLoadError(undefined);
    // The remembered choices are a convenience: if they cannot be read the launcher still opens, with its default.
    const remembered = layoutApi.get(endpoint).catch(() => null);
    // So are the agents found on this machine: if the catalog cannot be read the launcher lists the three adapters, as it did before it had one.
    const catalog = catalogApi.get(endpoint).catch(() => null);
    Promise.all([projectsApi.list(endpoint), profilesApi.get(endpoint), remembered, catalog])
      .then(([list, doc, layout, agents]) => current && setLoaded({ projects: list.map(toLauncherProject), doc, layout, catalog: agents }))
      .catch((e: unknown) => current && setLoadError(`Couldn't load the launcher: ${startFailure(e)}`));
    return () => {
      current = false;
    };
  }, [endpoint]);

  // A folder typed or browsed to that is not among the recent ones: listed first, and used like any other.
  const [extra, setExtra] = useState<Project | null>(null);
  // What the core said about the path typed in the folder field; null when the text is not a path.
  const [pathCheck, setPathCheck] = useState<PathCheck | null>(null);
  // True while the native folder dialog is open, so a second Ctrl+O or click opens no second one.
  const browsing = useRef(false);
  // Each check gets a number, so an answer for text typed earlier that arrives after the field changed again is dropped.
  const checks = useMemo(latestOnly, []);

  const recent = useMemo(() => loaded?.projects ?? [], [loaded]);
  const projects = useMemo(() => (extra && !recent.some((p) => p.id === extra.id) ? [extra, ...recent] : recent), [extra, recent]);
  const shells = useMemo(() => liveShells(loaded?.doc.profiles ?? []), [loaded]);
  // The most recent folder until one is picked.
  const projectId = projects.some((p) => p.id === choice.projectId) ? choice.projectId : (projects[0]?.id ?? "");
  // The shell and agent last started in this folder, unless the user has picked them here; a shell that is gone is replaced by the default.
  const defaultShell = loaded?.doc.default_id ?? "";
  const remembered = preselect(loaded?.layout ?? null, projectId, shells, defaultShell);
  const wantedShell = shellPicked ? choice.shell : (remembered?.shell ?? defaultShell);
  const shell = shells.some((x) => x.id === wantedShell) ? wantedShell : defaultShell;
  const shellFamily = shells.find((x) => x.id === shell)?.family;
  const programs = useMemo(() => launcherSetup(liveAgents(loaded?.catalog ?? null), shellFamily === "wsl", detections, detectionFailed), [loaded, shellFamily, detections, detectionFailed]);
  const offer = offerFor(programs, answers[shell], shell);
  // Only an agent the shell offers is shown or sent; one it lacks becomes Just the shell once the answer is in.
  const wantedAgent: LauncherChoice["agent"] = agentPicked ? choice.agent : remembered !== undefined ? remembered.agent : choice.agent;
  const agent = effectiveAgent({ agent: wantedAgent }, offer);
  const admin = adminChip(elevatedWanted, os, programById(agent ?? "", programs)?.adminUnavailable);
  const elevated = admin.on;
  useEffect(() => {
    if (!admin.enabled) setElevatedWanted(false);
  }, [admin.enabled]);
  // The worktree chip is offered only in a folder in git: leaving for another folder must not keep a flag the user can no longer see.
  const wantedMode = modePicked ? choice.mode : (preselectMode(loaded?.layout ?? null, projectId) ?? choice.mode);
  // Only a mode the agent offers is shown or sent; one it doesn't map falls back to the first it does.
  const mode = elevated ? "manual" : (offeredMode(programById(agent ?? "", programs)?.modes, wantedMode) ?? wantedMode);
  const worktree = choice.worktree && projects.find((p) => p.id === projectId)?.branch !== undefined;
  const shown: LauncherChoice = { ...choice, projectId, shell, agent, mode, worktree };
  const agentProgram = programById(agent ?? "", programs);
  const skipSetting = agentProgram?.skipsAll?.setting ?? (agentProgram?.adminSetupFailed ? "unknown" : undefined);
  const ackKey = elevated && skipSetting !== undefined ? `${agent}:${skipSetting}` : null;
  const bypassAcknowledged = ackKey !== null && bypassAck === ackKey;
  useEffect(() => setBypassAck(null), [ackKey]);

  // Each shell is asked once per opening, when it is first shown; a distro's answer can take a while, so Start waits for it.
  useEffect(() => {
    if (!endpoint || shell === "" || asked.current.has(shell)) return;
    asked.current.add(shell);
    profilesApi
      .agents(endpoint, shell)
      .then((a) => setAnswers((m) => ({ ...m, [a.profile_id]: a })))
      .catch(() => setAnswers((m) => ({ ...m, [shell]: "failed" })));
  }, [endpoint, shell]);

  // A pick leaves focus on its chip, where Enter reopens the list; and Start is disabled until the lists load. Command Prompt has no prompt line.
  const focusState: FocusState = { program: `${shown.shell}:${shown.agent ?? ""}`, prompt: agentProgram !== undefined && agentProgram.noFirst !== true && shellFamily !== "cmd", ready: loaded !== null };
  const lastFocus = useRef<FocusState | null>(null);
  useEffect(() => {
    const target = refocus(lastFocus.current, focusState);
    lastFocus.current = focusState;
    if (target) root.current?.querySelector<HTMLElement>(target === "prompt" ? ".launcher__prompt" : ".launcher__start")?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- focusState is rebuilt every render; its three fields are the dependencies.
  }, [focusState.program, focusState.prompt, focusState.ready]);

  // Check the typed path 150 ms after the last keystroke. Closing the field (query null) or unmounting cancels the timer and drops any answer on its way.
  useEffect(() => {
    const path = endpoint && query !== null ? typedPath(query) : null;
    if (path === null) {
      setPathCheck(null);
      return;
    }
    const n = checks.next();
    setPathCheck({ path, state: "checking" });
    const timer = setTimeout(() => {
      foldersApi
        .check(endpoint!, path)
        .then((ok) => ({ ok }), (e: unknown) => ({ status: e instanceof ApiError ? e.status : 0 }))
        .then((result) => checks.isLatest(n) && setPathCheck(checkState(path, result)));
    }, 150);
    return () => {
      clearTimeout(timer);
      checks.next();
    };
  }, [endpoint, query, checks]);
  // Never show a check for other text than what is in the field now (the effect above catches up a render later).
  const shownCheck = query !== null && pathCheck !== null && pathCheck.path === typedPath(query) ? pathCheck : null;

  const patch = (p: Partial<LauncherChoice>) => {
    if (p.shell !== undefined) setShellPicked(true);
    if (p.agent !== undefined) setAgentPicked(true);
    if (p.mode !== undefined) setModePicked(true);
    setChoice((c) => ({ ...c, ...p }));
    setError(undefined);
  };

  /** Use a folder the core has checked: its recent project if it is one, else a new entry; then pick it and close the field, as a pick does. */
  const takeFolder = (folder: FolderInfo) => {
    const chosen = chooseFolder(projects, folder);
    if (chosen.extra) setExtra(chosen.extra);
    patch({ projectId: chosen.projectId });
    setLists((l) => openFolder(l, null));
  };

  /** The native folder dialog, starting in the typed folder or else the chosen one. Cancelling leaves the field as it was. */
  const browse = async () => {
    if (!endpoint || browsing.current) return;
    browsing.current = true;
    const start = shownCheck?.state === "found" ? shownCheck.folder.path : (projects.find((p) => p.id === projectId)?.path ?? null);
    try {
      await browseFolder({
        start,
        pick: pickFolder,
        check: (path) => foldersApi.check(endpoint, path),
        take: takeFolder,
        fail: setError,
        focusField: () => root.current?.querySelector<HTMLInputElement>(".launcher__input")?.focus(),
      });
    } finally {
      browsing.current = false;
    }
  };

  // A shell takes no first prompt, so text typed for an agent before switching doesn't name its task.
  const taskName = taskNameFor(nameEdit, nameSource(shown, prompt, programs, shells));

  // Back from the confirm view puts focus where Enter starts again.
  const wasConfirming = useRef(false);
  useEffect(() => {
    if (wasConfirming.current && confirm === null && root.current) initialFocus(root.current)?.focus();
    wasConfirming.current = confirm !== null;
  }, [confirm]);

  /**
   * With the worktree chip on, Start first asks the core what it would run and shows it. After a refused create, `reason`
   * stays on screen above the fresh commands; a plan that is refused too shows its own reason instead.
   */
  const plan = (reason?: string) => {
    if (!endpoint) return;
    const seq = ++planSeq.current;
    setConfirm({ state: "loading" });
    setError(reason);
    // The repo's marshell.json is read beside the plan; a file that can't be read leaves the plan as it was, with no scripts.
    const projectPath = projects.find((p) => p.id === shown.projectId)?.path;
    const read = projectPath === undefined ? Promise.resolve(null) : getScripts(endpoint, projectPath).catch(() => null);
    Promise.all([tasksApi.plan(endpoint, taskRequest(shown, projects, taskName)), read])
      .then(([p, found]) => {
        if (seq !== planSeq.current) return;
        const scripts = confirmScripts(found);
        setConfirm({ state: "ready", commands: p.commands, ...(scripts === undefined ? {} : { scripts }) });
      })
      .catch((e: unknown) => {
        if (seq !== planSeq.current) return;
        setError(undefined);
        setConfirm({ state: "refused", reason: startFailure(e) });
      });
  };

  const back = () => {
    planSeq.current++;
    // The next plan may show other commands (another free name), so a task made for these ones must not be reused for them.
    tasks.forget();
    held.current = null;
    setConfirm(null);
    setError(undefined);
  };

  /** Frees the confirm view after a failure, or when it waits for an answer. */
  const release = () => {
    inFlight.current = false;
    frozen.current = undefined;
    setStarting(false);
  };

  /** Starts the agent session now, with the request built for this task. */
  const startAgent = async (request: CreateSessionRequest, text: string) => {
    const created = await createSession(request, () => sessionsApi.create(endpoint!, request));
    onStarted(created, text);
  };

  /** The task exists and its own marshell.json needs a yes before its setup runs: the card shows alone, and the two buttons are the only ways on. */
  const holdForAnswer = (task: Task, request: CreateSessionRequest, text: string, scripts: RepoScripts) => {
    held.current = { task, request, text, scripts };
    setConfirm({ state: "ready", commands: [], scripts: { kind: "ask", scripts, allowLabel: "Allow and run setup", cancelLabel: "Start without setup", only: true } });
    release();
  };

  /**
   * Runs the setup as a tab and keeps the agent's request for when it ends. A 409 (the worktree's file changed since it was read)
   * reads it again and asks again.
   */
  const launchSetup = async (task: Task, hash: string, request: CreateSessionRequest, text: string): Promise<void> => {
    try {
      const setup = await runScript(endpoint!, { cwd: task.path, script: "setup", hash, ...size() });
      onSetupStarted({ tabId: setup.tab_id, request, text });
      onStarted(setup, `Running setup in ${task.name}…`);
    } catch (e) {
      if (!(e instanceof ApiError && e.status === 409)) throw e;
      await settle(task, null, request, text, await getScripts(endpoint!, task.path).catch(() => null));
    }
  };

  /** What happens once the worktree's own marshell.json has been read (see `afterCreate`). */
  const settle = async (task: Task, shownHash: string | null, request: CreateSessionRequest, text: string, fresh: RepoScripts | null): Promise<void> => {
    const next = afterCreate(shownHash, fresh);
    if (next.kind === "run") return launchSetup(task, next.hash, request, text);
    if (next.kind === "ask") return holdForAnswer(task, request, text, next.scripts);
    announce(next.announce);
    return startAgent(request, text);
  };

  /**
   * Create and start: the worktree first, then the session in its folder. `how` is the button pressed: Create and start ("plain"),
   * Allow and create ("allow"), or Create without scripts ("skip"). The repo's setup runs only for a plain create with a trusted
   * setup, or after Allow; the agent then waits in the window until the setup tab ends.
   */
  const createAndStart = async (how: "plain" | "allow" | "skip" = "plain") => {
    // Only the commands on screen are sent; the core refuses them if they are no longer what it would run.
    const req = createRequest(taskRequest(shown, projects, taskName), confirm);
    if (!endpoint || !loaded || inFlight.current || req === null) return;
    const scripts = confirm?.state === "ready" ? confirm.scripts : undefined;
    // Allowing is always a press on the card's own button.
    if (scripts?.kind === "ask" && how === "plain") return;
    const problem = startProblem(shown, programs);
    if (problem !== undefined) {
      setError(problem);
      return;
    }
    const projectPath = projects.find((p) => p.id === shown.projectId)?.path;
    const setupWanted = how !== "skip" && scripts !== undefined && scripts.kind !== "problem" && (scripts.kind === "setup" || how === "allow") && scripts.scripts.setup !== null;
    inFlight.current = true;
    frozen.current = nextNumber;
    setStarting(true);
    setPhase("creating");
    setError(undefined);
    let made = false;
    try {
      if (how === "allow" && scripts?.kind === "ask") {
        // A file with a problem is never allowed.
        if (scripts.scripts.problem !== null || scripts.scripts.hash === null || projectPath === undefined) return release();
        await allowScripts(endpoint, projectPath, scripts.scripts.hash);
      }
      const task = await tasks.get(req, () => tasksApi.create(endpoint, req));
      made = true;
      setPhase("starting");
      const request = withColorScheme(launchRequest(shown, prompt, programs, loaded.doc, projects, size(), task.path, elevated), loaded.doc.profiles, appearance, appScheme());
      const folder = projects.find((p) => p.id === shown.projectId)?.name ?? "your home folder";
      const text = startText(folder);
      if (!setupWanted) return await startAgent(request, text);
      // The confirm view read the main checkout's file; the new worktree holds its own committed copy.
      await settle(task, scripts.scripts.hash, request, text, await getScripts(endpoint, task.path).catch(() => null));
    } catch (e) {
      release();
      // A refused create or allow (409: the commands or the file changed): say why, and show what would run now.
      if (!made && replansAfter(e)) plan(startFailure(e));
      else setError(startFailure(e));
    }
  };

  /** The card shown after the task was made: Allow and run setup, or Start without setup. */
  const continueHeld = async (allow: boolean) => {
    const h = held.current;
    if (!endpoint || h === null || inFlight.current) return;
    inFlight.current = true;
    frozen.current = nextNumber;
    setStarting(true);
    setPhase("starting");
    setError(undefined);
    try {
      if (!allow) return await startAgent(h.request, h.text);
      if (h.scripts.problem !== null || h.scripts.hash === null) return release();
      let hash = h.scripts.hash;
      try {
        hash = (await allowScripts(endpoint, h.task.path, hash)).hash ?? hash;
      } catch (e) {
        if (!(e instanceof ApiError && e.status === 409)) throw e;
        return await settle(h.task, null, h.request, h.text, await getScripts(endpoint, h.task.path).catch(() => null));
      }
      await launchSetup(h.task, hash, h.request, h.text);
    } catch (e) {
      release();
      setError(startFailure(e));
    }
  };

  /** What the new tab says until the process answers: the agent, or the shell alone. */
  const startText = (folder: string) => (shown.agent === null ? startingText(shells.find((x) => x.id === shown.shell)?.label ?? shown.shell, folder, []) : startingText(shown.agent, folder, programs));

  const start = async () => {
    if (!endpoint || !loaded || inFlight.current || (offer.state === "checking" && agent !== null) || (elevated && agentProgram?.adminSetupPending) || (ackKey !== null && !bypassAcknowledged)) return;
    // A program that can't start yet says why and sends nothing; no worktree is made for it either.
    const problem = startProblem(shown, programs);
    if (problem !== undefined) {
      setError(problem);
      return;
    }
    // Enter reaches here even while Start is disabled: an empty box starts nothing.
    if (wantsTask(shown, projects)) {
      plan();
      return;
    }
    inFlight.current = true;
    frozen.current = nextNumber;
    setStarting(true);
    setError(undefined);
    try {
      const request = launchRequest(shown, prompt, programs, loaded.doc, projects, size(), undefined, elevated);
      const sent = withColorScheme(request, loaded.doc.profiles, appearance, appScheme());
      const created = await createSession(sent, () => sessionsApi.create(endpoint, sent));
      const folder = projects.find((p) => p.id === shown.projectId)?.name ?? "your home folder";
      onStarted(created, startText(folder));
    } catch (e) {
      setError(startFailure(e));
      inFlight.current = false;
      frozen.current = undefined;
      setStarting(false);
    }
  };

  return (
    <div ref={root} className="live-app__launcher">
      <Launcher
        view={{ choice: shown, elevated, bypassAcknowledged, projects, query, pathCheck: shownCheck, activeIndex, shellsOpen, agentsOpen, prompt, nextNumber: shownNumber(nextNumber, starting, frozen.current), taskName, ...(confirm === null ? {} : { confirm }) }}
        shells={shells}
        agents={programs}
        offer={offer}
        paceChip={false}
        os={os}
        pending={starting}
        onElevated={setElevatedWanted}
        onBypassAck={(on) => setBypassAck(on ? ackKey : null)}
        error={error ?? loadError}
        busy={starting || loaded === null}
        busyText={elevated && phase === "starting" ? WAITING_FOR_ADMIN : progressText(phase, agentProgram?.label ?? shells.find((x) => x.id === shown.shell)?.label ?? shown.shell)}
        onChange={patch}
        onQuery={(q) => {
          setLists((l) => openFolder(l, q));
          setActiveIndex(0);
        }}
        onPick={(id) => {
          patch({ projectId: id });
          setLists((l) => openFolder(l, null));
        }}
        onUsePath={takeFolder}
        onBrowse={() => void browse()}
        onShells={(open) => {
          setLists((l) => openList(l, "shells", open));
          if (open) setActiveIndex(Math.max(0, shells.findIndex((x) => x.id === shown.shell)));
        }}
        onAgents={(open) => {
          setLists((l) => openList(l, "agents", open));
          // Row 0 is Just the shell, so an agent's row is one further on.
          if (open) setActiveIndex(Math.max(0, (offer.state === "ready" ? offer.offered : offer.state === "checking" ? programs : []).findIndex((p) => p.id === shown.agent) + 1));
        }}
        onActive={setActiveIndex}
        onPrompt={(text) => {
          setPrompt(text);
          setError(undefined);
        }}
        onStart={() => void start()}
        onTaskName={(text) => {
          setNameEdit(text);
          setError(undefined);
        }}
        onConfirm={() => void createAndStart()}
        onAllow={() => void (held.current === null ? createAndStart("allow") : continueHeld(true))}
        onSkipScripts={() => void (held.current === null ? createAndStart("skip") : continueHeld(false))}
        onBack={back}
        onClose={onClose}
      />
    </div>
  );
}
