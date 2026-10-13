//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Restore workflow views at the committed session boundary without observing presentation DOM. */
import { WorkflowViewPreferences } from '../application/WorkflowViewPreferences.js';
import type { WorkflowViewSource, WorkflowViewSnapshot } from '../domain/WorkflowView.js';
import type { WorkflowSessionStore } from '../../execution/application/WorkflowSessionStore.js';

/** Identify browser-local view metadata, separate from user workflow documents. */
export const WORKFLOW_VIEW_STORAGE_KEY = 'SugarCubes.WorkflowViews.v1';

/** Adapt storage and authoritative session commitment to one shared presentation owner. */
export class ComfyWorkflowViewPreferences implements WorkflowViewSource {
  readonly #views: WorkflowViewPreferences;
  readonly #listeners = new Set<() => void>();
  #stop: (() => void) | null = null;
  #ready = false;

  /** Inject browser persistence so invalid/unavailable storage never blocks workflow switching. */
  constructor(
    private readonly sessions: WorkflowSessionStore,
    storage: () => Pick<Storage, 'getItem' | 'setItem'> | null,
    private readonly defaultCanvas: () => boolean,
    logger: Pick<Console, 'warn'>,
  ) {
    let stored: unknown = null;
    try {
      const json = storage()?.getItem(WORKFLOW_VIEW_STORAGE_KEY);
      if (json && json.length <= 2_000_000) stored = JSON.parse(json);
    } catch (error: unknown) {
      logger.warn('SugarCubes could not restore workflow view preferences.', { error });
    }
    this.#views = new WorkflowViewPreferences(stored, (value) => {
      try {
        storage()?.setItem(WORKFLOW_VIEW_STORAGE_KEY, JSON.stringify(value));
      } catch (error: unknown) {
        logger.warn('SugarCubes could not persist workflow view preferences.', { error });
      }
    });
  }

  /** Read accepted per-workflow choices with no graph or storage access. */
  get snapshot(): WorkflowViewSnapshot {
    return this.#views.snapshot;
  }

  /** Expose graph commitment separately from the retained view choice during loading. */
  get ready(): boolean {
    return this.#ready;
  }

  /** Bind once after host setup and ignore intermediate asynchronous graph-load notifications. */
  setup(): void {
    if (this.#stop) return;
    this.#stop = this.sessions.subscribe(this.#synchronize);
    this.#synchronize();
  }

  /** Observe choices without requesting inventory or surface reconstruction. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Persist explicit actions only while the graph has a committed active workflow. */
  change(value: Partial<WorkflowViewSnapshot>): void {
    if (!this.sessions.read().active || !this.#views.change(value)) return;
    this.#publish();
  }

  /** Retire the host subscription without changing persisted choices. */
  dispose(): void {
    this.#stop?.();
    this.#stop = null;
    this.#listeners.clear();
  }

  /** Restore both view choices together only after Comfy commits the target workflow. */
  readonly #synchronize = (): void => {
    const snapshot = this.sessions.read();
    this.#views.retain(snapshot.open.map((context) => context.sessionId));
    const ready = !snapshot.loading && snapshot.active !== null;
    const changed = ready !== this.#ready;
    this.#ready = ready;
    if (!ready || !snapshot.active) {
      if (changed) this.#publish();
      return;
    }
    const context = snapshot.active;
    if (
      this.#views.activate({ id: context.sessionId, path: context.path }, this.defaultCanvas) ||
      changed
    )
      this.#publish();
  };

  /** Deliver one accepted selection/change signal to the canvas and masonry owners. */
  #publish(): void {
    for (const listener of this.#listeners) listener();
  }
}
