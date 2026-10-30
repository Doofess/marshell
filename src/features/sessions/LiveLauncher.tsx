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
import { catalogApi } from "../../lib/api/catalog";
import { ApiError } from "../../lib/api/client";
import { folders as foldersApi } from "../../lib/api/folders";
import { layout as layoutApi } from "../../lib/api/layout";
import { profiles as profilesApi } from "../../lib/api/profiles";
import { projects as projectsApi } from "../../lib/api/projects";
import { sessions as sessionsApi } from "../../lib/api/sessions";
import { tasksApi } from "../../lib/api/tasks";
import { toolsApi } from "../../lib/api/tools";
import type { ToolCatalog } from "../../generated/ToolCatalog";
import { pickFolder } from "../../lib/folderPicker";
import { useAppearance } from "../appearance/AppearanceProvider";
import { withColorScheme } from "../appearance/terminalSettings";
import type { Project } from "../launcher/filter";
import { Launcher, type TaskConfirm } from "../launcher/Launcher";
import { typedPath, type PathCheck } from "../launcher/folderRows";
import { openFolder, openPrograms, type OpenLists } from "../launcher/launcherKeys";
import { offeredMode } from "../modes/sessionModes";
import { taskNameFor } from "../tasks/taskName";
import { appScheme } from "../terminal/terminalTheme";
import { programById, startingText, type LauncherChoice } from "../launcher/launchChoice";
import { fieldsFilled, browseFolder, checkState, chooseFolder, latestOnly, createRequest, createThenLaunch, initialFocus, launchRequest, listedProgram, progressText, refocus, rememberTasks, replansAfter, type FocusState, livePrograms, nameSource, preselect, preselectMode, startFailure, startProblem, taskRequest, toLauncherProject, wantsTask } from "./launchPrograms";

// The worktree flag is part of this state and nothing else: it is never read from or written to the remembered choices, so the chip starts off every time.
const INITIAL: LauncherChoice = { program: "claude", projectId: "", mode: "manual", pace: "Balanced", worktree: false };

/** `layout` is read on every opening, so a folder used a moment ago is already remembered; null when it could not be read. */
type Loaded = { projects: Project[]; doc: ProfilesDoc; layout: LayoutDoc | null; catalog: AgentCatalog | null; tools: ToolCatalog | null };

type Props = {
  endpoint: Endpoint | null;
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
export function LiveLauncher({ endpoint, nextNumber, size, onStarted, onSetupStarted, announce, onClose }: Props) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const { appearance } = useAppearance();
  const [choice, setChoice] = useState(INITIAL);
  // Once the user picks a program here, the remembered one no longer applies.
  const [programPicked, setProgramPicked] = useState(false);
  const [modePicked, setModePicked] = useState(false);
  // One state for both lists, so opening one closes the other.
  const [lists, setLists] = useState<OpenLists>({ query: null, programsOpen: false });
  const { query, programsOpen } = lists;
  const [activeIndex, setActiveIndex] = useState(0);
  const [prompt, setPrompt] = useState("");
  const [error, setError] = useState<string>();
  // What was typed for a tool preset's fields, by field id. Kept here only: it goes into the create request and nowhere else.
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});
  // Why the lists did not load. Unlike `error` no edit clears it: Start stays disabled until they do.
  const [loadError, setLoadError] = useState<string>();
  const [starting, setStarting] = useState(false);
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
    // And the tool presets: without the list the launcher simply offers none.
    const tools = toolsApi.get(endpoint).catch(() => null);
    Promise.all([projectsApi.list(endpoint), profilesApi.get(endpoint), remembered, catalog, tools])
      .then(([list, doc, layout, agents, toolList]) => current && setLoaded({ projects: list.map(toLauncherProject), doc, layout, catalog: agents, tools: toolList }))
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
  const programs = useMemo(() => livePrograms(loaded?.catalog ?? null, loaded?.doc.profiles ?? [], loaded?.tools ?? null), [loaded]);
  // The most recent folder until one is picked.
  const projectId = projects.some((p) => p.id === choice.projectId) ? choice.projectId : (projects[0]?.id ?? "");
  // The program last started in this folder, unless the user has picked one themselves; an agent that is not listed (not installed) is replaced by the first that is.
  const program = listedProgram(programPicked ? choice.program : (preselect(loaded?.layout ?? null, projectId, programs) ?? choice.program), programs);
  // The worktree chip is offered only in a folder in git: leaving for another folder must not keep a flag the user can no longer see.
  const wantedMode = modePicked ? choice.mode : (preselectMode(loaded?.layout ?? null, projectId) ?? choice.mode);
  // Only a mode the agent offers is shown or sent; one it doesn't map falls back to the first it does.
  const mode = offeredMode(programById(program, programs)?.modes, wantedMode) ?? wantedMode;
  const worktree = choice.worktree && projects.find((p) => p.id === projectId)?.branch !== undefined;
  const shown: LauncherChoice = { ...choice, projectId, program, mode, worktree };

  // A pick leaves focus on the program chip, where Enter reopens the list; and Start is disabled until the lists load.
  const focusState: FocusState = { program: shown.program, prompt: !programById(shown.program, programs)?.noFirst, ready: loaded !== null };
  const lastFocus = useRef<FocusState | null>(null);
  useEffect(() => {
    const target = refocus(lastFocus.current, focusState);
    lastFocus.current = focusState;
    // A tool preset with boxes to fill takes the cursor in its first box; Start would be disabled anyway.
    const firstBox = programById(focusState.program, programs)?.fields?.length ? ".launcher__field-input" : undefined;
    if (target) root.current?.querySelector<HTMLElement>(target === "prompt" ? ".launcher__prompt" : (firstBox ?? ".launcher__start"))?.focus();
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
    if (p.program !== undefined) {
      setProgramPicked(true);
      setFieldValues({});
    }
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

  const chosen = programById(shown.program, programs);
  const toolFields = chosen?.fields ?? [];
  const filled = fieldsFilled(chosen, fieldValues);
  const setField = (id: string, value: string) => {
    setFieldValues((v) => ({ ...v, [id]: value }));
    setError(undefined);
  };
  const fieldBoxes =
    toolFields.length === 0 ? undefined : (
      <>
        {toolFields.map((f, i) => (
          <div key={f.id} className="launcher__task">
            <label className="launcher__task-label" htmlFor={`launcher-field-${f.id}`}>
              {f.label}
            </label>
            <input
              id={`launcher-field-${f.id}`}
              className="launcher__task-input launcher__field-input"
              type="text"
              value={fieldValues[f.id] ?? ""}
              spellCheck={false}
              autoComplete="off"
              onChange={(e) => setField(f.id, e.target.value)}
              onKeyDown={(e) => {
                // Enter in a box other than the last moves on; in the last one it reaches the launcher, which starts.
                if (e.key !== "Enter" || e.shiftKey || i === toolFields.length - 1) return;
                e.preventDefault();
                e.stopPropagation();
                root.current?.querySelectorAll<HTMLElement>(".launcher__field-input")[i + 1]?.focus();
              }}
            />
          </div>
        ))}
      </>
    );

  // A shell takes no first prompt, so text typed for an agent before switching doesn't name its task.
  const taskName = taskNameFor(nameEdit, nameSource(shown, prompt, programs));

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
    setStarting(false);
  };

  /** Starts the agent session now, with the request built for this task. */
  const startAgent = async (request: CreateSessionRequest, text: string) => {
    const created = await sessionsApi.create(endpoint!, request);
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
      const request = withColorScheme(launchRequest(shown, prompt, programs, loaded.doc, projects, size(), task.path, fieldValues), loaded.doc.profiles, appearance, appScheme());
      const folder = projects.find((p) => p.id === shown.projectId)?.name ?? "your home folder";
      const text = startingText(shown.program, folder, programs);
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

  const start = async () => {
    if (!endpoint || !loaded || inFlight.current) return;
    // A program that can't start yet says why and sends nothing; no worktree is made for it either.
    const problem = startProblem(shown, programs);
    if (problem !== undefined) {
      setError(problem);
      return;
    }
    // Enter reaches here even while Start is disabled: an empty box starts nothing.
    if (!filled) return;
    if (wantsTask(shown, projects)) {
      plan();
      return;
    }
    inFlight.current = true;
    setStarting(true);
    setError(undefined);
    try {
      const request = launchRequest(shown, prompt, programs, loaded.doc, projects, size(), undefined, fieldValues);
      const created = await sessionsApi.create(endpoint, withColorScheme(request, loaded.doc.profiles, appearance, appScheme()));
      const folder = projects.find((p) => p.id === shown.projectId)?.name ?? "your home folder";
      onStarted(created, startingText(shown.program, folder, programs));
    } catch (e) {
      setError(startFailure(e));
      inFlight.current = false;
      setStarting(false);
    }
  };

  return (
    <div ref={root} className="live-app__launcher">
      <Launcher
        view={{ choice: shown, projects, query, pathCheck: shownCheck, activeIndex, programsOpen, prompt, nextNumber, taskName, ...(confirm === null ? {} : { confirm }) }}
        programs={programs}
        paceChip={false}
        error={error ?? loadError}
        busy={starting || loaded === null || !filled}
        fields={fieldBoxes}
        busyText={progressText(phase, programById(shown.program, programs)?.label ?? shown.program)}
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
        onPrograms={(open) => {
          setLists((l) => openPrograms(l, open));
          if (open) setActiveIndex(Math.max(0, programs.findIndex((p) => p.id === shown.program)));
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
