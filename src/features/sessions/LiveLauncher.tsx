import { useEffect, useMemo, useRef, useState } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { Task } from "../../generated/Task";
import { layout as layoutApi } from "../../lib/api/layout";
import { profiles as profilesApi } from "../../lib/api/profiles";
import { projects as projectsApi } from "../../lib/api/projects";
import { sessions as sessionsApi } from "../../lib/api/sessions";
import { tasksApi } from "../../lib/api/tasks";
import { useAppearance } from "../appearance/AppearanceProvider";
import { withColorScheme } from "../appearance/terminalSettings";
import type { Project } from "../launcher/filter";
import { Launcher, type TaskConfirm } from "../launcher/Launcher";
import { taskNameFor } from "../tasks/taskName";
import { appScheme } from "../terminal/terminalTheme";
import { programById, startingText, type LauncherChoice } from "../launcher/launchChoice";
import { createThenLaunch, initialFocus, launchRequest, refocus, type FocusState, livePrograms, preselect, startFailure, taskRequest, toLauncherProject, wantsTask } from "./launchPrograms";

// The worktree flag is part of this state and nothing else: it is never read from or written to the remembered choices, so the chip starts off every time.
const INITIAL: LauncherChoice = { program: "claude", projectId: "", mode: "Manual", pace: "Balanced", worktree: false };

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
  const [query, setQuery] = useState<string | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const [programsOpen, setProgramsOpen] = useState(false);
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
  // A task already made for this request: if the session then fails to start, trying again must not make a second worktree.
  const made = useRef<{ key: string; task: Task } | null>(null);
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

  const projects = useMemo(() => loaded?.projects ?? [], [loaded]);
  const programs = useMemo(() => livePrograms(loaded?.doc.profiles ?? []), [loaded]);
  // The most recent folder until one is picked.
  const projectId = projects.some((p) => p.id === choice.projectId) ? choice.projectId : (projects[0]?.id ?? "");
  // The program last started in this folder, unless the user has picked one themselves.
  const program = programPicked ? choice.program : (preselect(loaded?.layout ?? null, projectId, programs) ?? choice.program);
  // The worktree chip is offered only in a folder in git: leaving for another folder must not keep a flag the user can no longer see.
  const worktree = choice.worktree && projects.find((p) => p.id === projectId)?.branch !== undefined;
  const shown: LauncherChoice = { ...choice, projectId, program, worktree };

  // A pick leaves focus on the program chip, where Enter reopens the list; and Start is disabled until the lists load.
  const focusState: FocusState = { program: shown.program, prompt: !programById(shown.program, programs)?.noFirst, ready: loaded !== null };
  const lastFocus = useRef<FocusState | null>(null);
  useEffect(() => {
    const target = refocus(lastFocus.current, focusState);
    lastFocus.current = focusState;
    if (target) root.current?.querySelector<HTMLElement>(target === "prompt" ? ".launcher__prompt" : ".launcher__start")?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- focusState is rebuilt every render; its three fields are the dependencies.
  }, [focusState.program, focusState.prompt, focusState.ready]);

  const patch = (p: Partial<LauncherChoice>) => {
    if (p.program !== undefined) setProgramPicked(true);
    setChoice((c) => ({ ...c, ...p }));
    setError(undefined);
  };

  const taskName = taskNameFor(nameEdit, prompt);

  // Back from the confirm view puts focus where Enter starts again.
  const wasConfirming = useRef(false);
  useEffect(() => {
    if (wasConfirming.current && confirm === null && root.current) initialFocus(root.current)?.focus();
    wasConfirming.current = confirm !== null;
  }, [confirm]);

  /** With the worktree chip on, Start first asks the core what it would run and shows it. */
  const plan = () => {
    if (!endpoint) return;
    const seq = ++planSeq.current;
    setConfirm({ state: "loading" });
    setError(undefined);
    tasksApi
      .plan(endpoint, taskRequest(shown, projects, taskName))
      .then((p) => seq === planSeq.current && setConfirm({ state: "ready", commands: p.commands }))
      .catch((e: unknown) => seq === planSeq.current && setConfirm({ state: "refused", reason: startFailure(e) }));
  };

  const back = () => {
    planSeq.current++;
    setConfirm(null);
    setError(undefined);
  };

  /** Create and start: the worktree first, then the session in its folder. */
  const createAndStart = async () => {
    if (!endpoint || !loaded || inFlight.current) return;
    inFlight.current = true;
    setStarting(true);
    setError(undefined);
    const req = taskRequest(shown, projects, taskName);
    const key = JSON.stringify(req);
    try {
      const created = await createThenLaunch(
        async () => {
          if (made.current?.key === key) return made.current.task;
          const task = await tasksApi.create(endpoint, req);
          made.current = { key, task };
          return task;
        },
        (task) => sessionsApi.create(endpoint, withColorScheme(launchRequest(shown, prompt, programs, loaded.doc, projects, size(), task.path), loaded.doc.profiles, appearance, appScheme())),
      );
      const folder = projects.find((p) => p.id === shown.projectId)?.name ?? "your home folder";
      onStarted(created, startingText(shown.program, folder, programs));
    } catch (e) {
      setError(startFailure(e));
      inFlight.current = false;
      setStarting(false);
    }
  };

  const start = async () => {
    if (!endpoint || !loaded || inFlight.current) return;
    if (wantsTask(shown, projects, programs)) {
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
        view={{ choice: shown, projects, query, activeIndex, programsOpen, prompt, nextNumber, taskName, ...(confirm === null ? {} : { confirm }) }}
        programs={programs}
        chips={false}
        error={error ?? loadError}
        busy={starting || loaded === null}
        onChange={patch}
        onQuery={(q) => {
          setQuery(q);
          setActiveIndex(0);
        }}
        onPick={(id) => {
          patch({ projectId: id });
          setQuery(null);
        }}
        onPrograms={(open) => {
          setProgramsOpen(open);
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
