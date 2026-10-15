import type { Endpoint } from "../../generated/Endpoint";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { profiles } from "../../lib/api/profiles";
import { changeCount, deleteProfile, duplicateProfile, forSave, makeDefault, resetProfile, updateProfile, type ProfilePatch } from "./profilesDoc";

type Api = Pick<typeof profiles, "get" | "detected" | "put">;

export type ProfilesState = {
  phase: "loading" | "ready" | "failed";
  /** Why the profiles did not load. */
  loadError: string | null;
  /** What the core has. */
  saved: ProfilesDoc | null;
  /** What the form shows: the saved document plus the edits not saved yet. */
  draft: ProfilesDoc | null;
  /** The shells as detection found them: what Reset goes back to, and the profiles that cannot be deleted. */
  detected: Profile[];
  selectedId: string;
  saving: boolean;
  /** The core's answer to a refused save, word for word. */
  error: string | null;
  /** The last save worked and nothing has changed since. */
  justSaved: boolean;
};

const INITIAL: ProfilesState = { phase: "loading", loadError: null, saved: null, draft: null, detected: [], selectedId: "", saving: false, error: null, justSaved: false };

const reason = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** Whether there is anything to save. Environment rows without a name do not count: they are not sent. */
export const isDirty = (s: ProfilesState): boolean => s.saved !== null && s.draft !== null && changeCount(s.saved, forSave(s.draft)) > 0;

/**
 * The profiles page's state: load, edit in memory, save on request. Nothing is sent until `save`. Plain class so it is tested
 * without a DOM; ProfilesPage wraps it with useSyncExternalStore.
 */
export class ProfilesStore {
  private state: ProfilesState = INITIAL;
  private listeners = new Set<() => void>();

  constructor(
    private api: Api,
    private endpoint: Endpoint,
  ) {}

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getState = (): ProfilesState => this.state;

  private set(patch: Partial<ProfilesState>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l());
  }

  /** Edits the draft; any earlier refusal or "Saved." is stale after that. */
  private edit(draft: ProfilesDoc, selectedId = this.state.selectedId): void {
    this.set({ draft, selectedId, error: null, justSaved: false });
  }

  async load(): Promise<void> {
    this.set({ ...INITIAL });
    try {
      const [doc, detected] = await Promise.all([this.api.get(this.endpoint), this.api.detected(this.endpoint)]);
      this.set({ phase: "ready", saved: doc, draft: doc, detected, selectedId: doc.profiles.find((p) => p.id === doc.default_id)?.id ?? doc.profiles[0]?.id ?? "" });
    } catch (e) {
      this.set({ phase: "failed", loadError: reason(e) });
    }
  }

  select(id: string): void {
    this.set({ selectedId: id });
  }

  /** Edits the selected profile. */
  patch(patch: ProfilePatch): void {
    const { draft, selectedId } = this.state;
    if (draft) this.edit(updateProfile(draft, selectedId, patch));
  }

  /** Adds a copy of the selected profile and selects it. */
  add(): void {
    const { draft, selectedId } = this.state;
    if (!draft) return;
    const copy = duplicateProfile(draft, selectedId);
    this.edit(copy.doc, copy.id);
  }

  /** Deletes the selected profile unless it is a detected one. */
  remove(): void {
    const { draft, selectedId, detected } = this.state;
    if (!draft || draft.profiles.length < 2) return;
    const next = deleteProfile(draft, selectedId, detected);
    if (next === draft) return;
    const at = draft.profiles.findIndex((p) => p.id === selectedId);
    this.edit(next, (next.profiles[at] ?? next.profiles[at - 1] ?? next.profiles[0])?.id ?? "");
  }

  makeDefault(): void {
    const { draft, selectedId } = this.state;
    if (draft) this.edit(makeDefault(draft, selectedId));
  }

  /** Puts the selected detected profile back to what detection found. */
  reset(): void {
    const { draft, selectedId, detected } = this.state;
    if (draft) this.edit(resetProfile(draft, selectedId, detected));
  }

  /** Throws the edits away. */
  discard(): void {
    const { saved, selectedId } = this.state;
    if (!saved) return;
    this.set({ draft: saved, selectedId: saved.profiles.some((p) => p.id === selectedId) ? selectedId : (saved.profiles[0]?.id ?? ""), error: null, justSaved: false });
  }

  /** Sends the draft. A refusal (400 for a document that fails validation, 500 for a file the core cannot write) stays as text in `error`. */
  async save(): Promise<void> {
    const { draft, saving, selectedId } = this.state;
    if (!draft || saving) return;
    this.set({ saving: true, error: null, justSaved: false });
    try {
      const doc = await this.api.put(this.endpoint, forSave(draft));
      this.set({ saving: false, saved: doc, draft: doc, selectedId: doc.profiles.some((p) => p.id === selectedId) ? selectedId : (doc.profiles[0]?.id ?? ""), justSaved: true });
    } catch (e) {
      this.set({ saving: false, error: reason(e) });
    }
  }
}
