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
/** Own every SugarCubes contact with Comfy's workflow change tracker. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import { WORKFLOW_VIEW_EXTRA, type WorkflowViewKind } from './ComfyWorkflowViewDocument.js';

type UndoRedo = (this: unknown, event: unknown) => unknown;

/**
 * Own SugarCubes' contact with Comfy's workflow change tracker: shortcuts another surface consumed
 * stay out of workflow history, and the view a workflow records is written without becoming a
 * step in that history.
 *
 * Comfy listens for keys on the window in the capture phase, before any surface inside the
 * document, and walks workflow history a frame later whatever that surface did with the key. A
 * surface with its own history, such as the editing canvas, consumes the shortcut by preventing
 * its default; without this adapter the workflow would then be rolled back as well, reloading the
 * graph underneath the surface. The adapter wraps the history owner's shortcut entry point once
 * per tracker class so a consumed shortcut is reported as handled and changes nothing.
 */
export class ComfyWorkflowChangeTracker {
  readonly #wrapped = new Map<UnknownRecord, UndoRedo>();
  #active = false;
  #reported = false;
  #viewReported = false;

  /** Borrow the host app and window; `report` receives a host that no longer fits. */
  constructor(
    private readonly app: unknown,
    private readonly window: Window,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Start guarding; the tracker is adopted lazily because workflows open after setup. */
  install(): void {
    if (this.#active) return;
    this.#active = true;
    // Comfy's own listener is registered first, and defers its history walk to the next frame,
    // so adopting the active tracker here always precedes that walk.
    this.window.addEventListener('keydown', this.#adopt, true);
  }

  /** Restore every tracker class this adapter wrapped. */
  dispose(): void {
    if (!this.#active) return;
    this.#active = false;
    this.window.removeEventListener('keydown', this.#adopt, true);
    for (const [prototype, original] of this.#wrapped) prototype.undoRedo = original;
    this.#wrapped.clear();
  }

  readonly #adopt = (): void => {
    const prototype = this.#trackerClass();
    if (!prototype || this.#wrapped.has(prototype)) return;
    const original = prototype.undoRedo;
    if (typeof original !== 'function') {
      if (!this.#reported)
        this.report(new TypeError('Comfy workflow history shortcuts are unavailable.'));
      this.#reported = true;
      return;
    }
    const undoRedo = original as UndoRedo;
    this.#wrapped.set(prototype, undoRedo);
    prototype.undoRedo = function guarded(this: unknown, event: unknown): unknown {
      // Comfy treats a truthy result as handled and records no workflow change for the key.
      if (event instanceof Event && event.defaultPrevented) return Promise.resolve(true);
      return undoRedo.call(this, event);
    };
  };

  /**
   * Record in the active workflow which view it is showing, without an undo step.
   *
   * Comfy's undo reloads a stored copy of the workflow, and its unsaved mark compares the copy it
   * opened with the current one. The view is written into the live graph, the current copy and
   * every stored undo and redo copy, so no step is recorded, undoing other edits keeps the view,
   * and the workflow shows unsaved until it is saved. Returns false, reported once, when the
   * tracker no longer has that shape; the view is then kept only for this session.
   */
  recordView(view: WorkflowViewKind): boolean {
    const tracker = this.#tracker();
    const graph = isRecord(this.app) ? this.app.rootGraph : null;
    const queues = tracker ? [tracker.undoQueue, tracker.redoQueue] : [];
    if (
      !tracker ||
      !isRecord(graph) ||
      !isRecord(tracker.activeState) ||
      !queues.every(Array.isArray) ||
      typeof tracker.updateModified !== 'function'
    ) {
      if (!this.#viewReported)
        this.report(new TypeError('Comfy workflow view recording is unavailable.'));
      this.#viewReported = true;
      return false;
    }
    const viewed = (extra: unknown): UnknownRecord => ({
      ...(isRecord(extra) ? extra : {}),
      [WORKFLOW_VIEW_EXTRA]: view,
    });
    // Stored copies are replaced, never changed in place: on a workflow with no edits Comfy's
    // opened copy is the very object it holds as current, and it must keep its own view.
    const copy = (state: unknown): unknown =>
      isRecord(state) ? { ...state, extra: viewed(state.extra) } : state;
    graph.extra = viewed(graph.extra);
    tracker.activeState = copy(tracker.activeState);
    for (const queue of queues as unknown[][])
      for (const [index, state] of queue.entries()) queue[index] = copy(state);
    Reflect.apply(tracker.updateModified, tracker, []);
    return true;
  }

  /** The active workflow's history owner. */
  #tracker(): UnknownRecord | null {
    const manager =
      isRecord(this.app) && isRecord(this.app.extensionManager) ? this.app.extensionManager : null;
    const store = manager && isRecord(manager.workflow) ? manager.workflow : null;
    const workflow = store && isRecord(store.activeWorkflow) ? store.activeWorkflow : null;
    return workflow && isRecord(workflow.changeTracker) ? workflow.changeTracker : null;
  }

  /** The class of the active workflow's history owner, shared by every open workflow. */
  #trackerClass(): UnknownRecord | null {
    const tracker = this.#tracker();
    const prototype: unknown = tracker ? Object.getPrototypeOf(tracker) : null;
    return isRecord(prototype) && prototype !== Object.prototype ? prototype : null;
  }
}
