import { useEffect, useMemo, useRef, useState } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { FolderInfo } from "../../generated/FolderInfo";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { ApiError } from "../../lib/api/client";
import { folders as foldersApi } from "../../lib/api/folders";
import { layout as layoutApi } from "../../lib/api/layout";
import { profiles as profilesApi } from "../../lib/api/profiles";
import { projects as projectsApi } from "../../lib/api/projects";
import { sessions as sessionsApi } from "../../lib/api/sessions";
import { tasksApi } from "../../lib/api/tasks";
import { pickFolder } from "../../lib/folderPicker";
import { useAppearance } from "../appearance/AppearanceProvider";
import { withColorScheme } from "../appearance/terminalSettings";
import type { Project } from "../launcher/filter";
import { Launcher, type TaskConfirm } from "../launcher/Launcher";
import { typedPath, type PathCheck } from "../launcher/folderRows";
import { openFolder, openPrograms, type OpenLists } from "../launcher/launcherKeys";
import { taskNameFor } from "../tasks/taskName";
import { appScheme } from "../terminal/terminalTheme";
import { programById, startingText, type LauncherChoice } from "../launcher/launchChoice";
import { checkState, chooseFolder, latestOnly, createRequest, createThenLaunch, initialFocus, launchRequest, progressText, refocus, rememberTasks, replansAfter, type FocusState, livePrograms, nameSource, preselect, preselectMode, startFailure, taskRequest, toLauncherProject, wantsTask } from "./launchPrograms";

// The worktree flag is part of this state and nothing else: it is never read from or written to the remembered choices, so the chip starts off every time.
const INITIAL: LauncherChoice = { program: "claude", projectId: "", mode: "manual", pace: "Balanced", worktree: false };

/** `layout` is read on every opening, so a folder used a moment ago is already remembered; null when it could not be read. */
type Loaded = { projects: Project[]; doc: ProfilesDoc; layout: LayoutDoc | null };

type Props = {
  endpoint: Endpoint | null;
  /** The number the new session takes. */
  nextNumber: number;
  /** The last measured terminal size, read when Start is pressed. */
  size: () => { cols: number; rows: number };
  /** The core created the session. `text` is what its tab says until the first output. */
  onStarted: (created: CreateSessionResponse, text: string) => void;
  onClose: () => void;
};

/**
 * The launcher on the core: it lists the recent folders, the agents and the shell profiles, and Start creates the session.
 * Mounted only while open, so every opening starts fresh. When the core refuses, its text shows under Start and the launcher stays.
 */
export function LiveLauncher({ endpoint, nextNumber, size, onStarted, onClose }: Props) {
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
    Promise.all([projectsApi.list(endpoint), profilesApi.get(endpoint), remembered])
      .then(([list, doc, layout]) => current && setLoaded({ projects: list.map(toLauncherProject), doc, layout }))
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
  const programs = useMemo(() => livePrograms(loaded?.doc.profiles ?? []), [loaded]);
  // The most recent folder until one is picked.
  const projectId = projects.some((p) => p.id === choice.projectId) ? choice.projectId : (projects[0]?.id ?? "");
  // The program last started in this folder, unless the user has picked one themselves.
  const program = programPicked ? choice.program : (preselect(loaded?.layout ?? null, projectId, programs) ?? choice.program);
  // The worktree chip is offered only in a folder in git: leaving for another folder must not keep a flag the user can no longer see.
  const mode = modePicked ? choice.mode : (preselectMode(loaded?.layout ?? null, projectId) ?? choice.mode);
  const worktree = choice.worktree && projects.find((p) => p.id === projectId)?.branch !== undefined;
  const shown: LauncherChoice = { ...choice, projectId, program, mode, worktree };

  // A pick leaves focus on the program chip, where Enter reopens the list; and Start is disabled until the lists load.
  const focusState: FocusState = { program: shown.program, prompt: !programById(shown.program, programs)?.noFirst, ready: loaded !== null };
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
    if (p.program !== undefined) setProgramPicked(true);
    if (p.mode !== undefined) setModePicked(true);
    setChoice((c) => ({ ...c, ...p }));
    setError(undefined);
  };

  /** Use a folder the core has checked: its recent project if it is one, else a new entry; then pick it and close the field, as a pick does. */
  const useFolder = (folder: FolderInfo) => {
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
      let picked: string | null;
      try {
        picked = await pickFolder(start);
      } catch (e) {
        setError(`Couldn't open the folder picker: ${e instanceof Error ? e.message : String(e)}`);
        return;
      }
      if (picked === null) {
        root.current?.querySelector<HTMLInputElement>(".launcher__input")?.focus();
        return;
      }
      let folder: FolderInfo;
      try {
        folder = await foldersApi.check(endpoint, picked);
      } catch (e) {
        setError(`Couldn't use ${picked}: ${e instanceof Error ? e.message : String(e)}`);
        return;
      }
      useFolder(folder);
    } finally {
      browsing.current = false;
    }
  };

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
    tasksApi
      .plan(endpoint, taskRequest(shown, projects, taskName))
      .then((p) => seq === planSeq.current && setConfirm({ state: "ready", commands: p.commands }))
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
    setConfirm(null);
    setError(undefined);
  };

  /** Create and start: the worktree first, then the session in its folder. */
  const createAndStart = async () => {
    // Only the commands on screen are sent; the core refuses them if they are no longer what it would run.
    const req = createRequest(taskRequest(shown, projects, taskName), confirm);
    if (!endpoint || !loaded || inFlight.current || req === null) return;
    inFlight.current = true;
    setStarting(true);
    setPhase("creating");
    setError(undefined);
    let made = false;
    try {
      const created = await createThenLaunch(
        () => tasks.get(req, () => tasksApi.create(endpoint, req)),
        (task) => {
          made = true;
          setPhase("starting");
          return sessionsApi.create(endpoint, withColorScheme(launchRequest(shown, prompt, programs, loaded.doc, projects, size(), task.path), loaded.doc.profiles, appearance, appScheme()));
        },
      );
      const folder = projects.find((p) => p.id === shown.projectId)?.name ?? "your home folder";
      onStarted(created, startingText(shown.program, folder, programs));
    } catch (e) {
      inFlight.current = false;
      setStarting(false);
      // A refused create: say why, and show what the core would run now.
      if (!made && replansAfter(e)) plan(startFailure(e));
      else setError(startFailure(e));
    }
  };

  const start = async () => {
    if (!endpoint || !loaded || inFlight.current) return;
    if (wantsTask(shown, projects)) {
      plan();
      return;
    }
    inFlight.current = true;
    setStarting(true);
    setError(undefined);
    try {
      const request = launchRequest(shown, prompt, programs, loaded.doc, projects, size());
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
        busy={starting || loaded === null}
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
        onUsePath={useFolder}
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
        onBack={back}
        onClose={onClose}
      />
    </div>
  );
}
