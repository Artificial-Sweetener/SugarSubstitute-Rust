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
/** Keep shortcuts another surface consumed out of Comfy's workflow history. */
import { isRecord, type UnknownRecord } from '../types/common.js';

type UndoRedo = (this: unknown, event: unknown) => unknown;

/**
 * Make Comfy's workflow history respect a consumed undo or redo shortcut.
 *
 * Comfy listens for keys on the window in the capture phase, before any surface inside the
 * document, and walks workflow history a frame later whatever that surface did with the key. A
 * surface with its own history, such as the editing canvas, consumes the shortcut by preventing
 * its default; without this adapter the workflow would then be rolled back as well, reloading the
 * graph underneath the surface. The adapter wraps the history owner's shortcut entry point once
 * per tracker class so a consumed shortcut is reported as handled and changes nothing.
 */
export class ComfyWorkflowHistoryShortcuts {
  readonly #wrapped = new Map<UnknownRecord, UndoRedo>();
  #active = false;
  #reported = false;

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

  /** The class of the active workflow's history owner, shared by every open workflow. */
  #trackerClass(): UnknownRecord | null {
    const manager =
      isRecord(this.app) && isRecord(this.app.extensionManager) ? this.app.extensionManager : null;
    const store = manager && isRecord(manager.workflow) ? manager.workflow : null;
    const workflow = store && isRecord(store.activeWorkflow) ? store.activeWorkflow : null;
    const tracker = workflow && isRecord(workflow.changeTracker) ? workflow.changeTracker : null;
    const prototype: unknown = tracker ? Object.getPrototypeOf(tracker) : null;
    return isRecord(prototype) && prototype !== Object.prototype ? prototype : null;
  }
}
