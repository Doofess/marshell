import { useEffect, useMemo, useRef, useState } from "react";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { profiles as profilesApi } from "../../lib/api/profiles";
import { projects as projectsApi } from "../../lib/api/projects";
import { sessions as sessionsApi } from "../../lib/api/sessions";
import type { Project } from "../launcher/filter";
import { Launcher } from "../launcher/Launcher";
import { startingText, type LauncherChoice } from "../launcher/launchChoice";
import { launchRequest, livePrograms, startFailure, toLauncherProject } from "./launchPrograms";

const INITIAL: LauncherChoice = { program: "claude", projectId: "", mode: "Manual", pace: "Balanced", worktree: false };

type Loaded = { projects: Project[]; doc: ProfilesDoc };

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
  const [choice, setChoice] = useState(INITIAL);
  const [query, setQuery] = useState<string | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const [programsOpen, setProgramsOpen] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [error, setError] = useState<string>();
  const [starting, setStarting] = useState(false);
  const inFlight = useRef(false);
  const root = useRef<HTMLDivElement>(null);

  // Keys go to the launcher, not the terminal behind it; closing hands focus back if that terminal is still there.
  useEffect(() => {
    const before = document.activeElement;
    root.current?.querySelector<HTMLElement>(".launcher__prompt, .launcher__chip")?.focus();
    return () => {
      if (before instanceof HTMLElement && before !== document.body && before.isConnected) before.focus();
    };
  }, []);

  useEffect(() => {
    if (!endpoint) return;
    let current = true;
    setLoaded(null);
    Promise.all([projectsApi.list(endpoint), profilesApi.get(endpoint)])
      .then(([list, doc]) => current && setLoaded({ projects: list.map(toLauncherProject), doc }))
      .catch((e: unknown) => current && setError(`Couldn't load the launcher: ${startFailure(e)}`));
    return () => {
      current = false;
    };
  }, [endpoint]);

  const projects = useMemo(() => loaded?.projects ?? [], [loaded]);
  const programs = useMemo(() => livePrograms(loaded?.doc.profiles ?? []), [loaded]);
  // The most recent folder until one is picked.
  const shown: LauncherChoice = { ...choice, projectId: projects.some((p) => p.id === choice.projectId) ? choice.projectId : (projects[0]?.id ?? "") };

  const patch = (p: Partial<LauncherChoice>) => {
    setChoice((c) => ({ ...c, ...p }));
    setError(undefined);
  };

  const start = async () => {
    if (!endpoint || !loaded || inFlight.current) return;
    inFlight.current = true;
    setStarting(true);
    setError(undefined);
    try {
      const created = await sessionsApi.create(endpoint, launchRequest(shown, prompt, programs, loaded.doc, projects, size()));
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
        view={{ choice: shown, projects, query, activeIndex, programsOpen, prompt, nextNumber }}
        programs={programs}
        chips={false}
        error={error}
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
        onPrompt={setPrompt}
        onStart={() => void start()}
        onClose={onClose}
      />
    </div>
  );
}
