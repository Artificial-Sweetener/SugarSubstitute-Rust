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
/** Connect native graph transactions to Comfy's active workflow history owner. */
import { isRecord, type UnknownRecord } from '../types/common.js';

interface HistoryEntry {
  finish(): void;
}

/** Keep graph mutation clients independent of Comfy's renderer-specific history registration. */
export class ComfyCanvasHistoryAdapter {
  #configuring = false;
  #active = false;
  readonly #bindings: Array<() => void> = [];

  /** Connect native history and dependent reconciliation to this document's lifetime. */
  constructor(
    private readonly app: unknown,
    private readonly document: Document,
    private readonly beforeCommit: () => void,
  ) {}

  /** Preserve native callback chains while supplying workflow capture for graph transactions. */
  install(): void {
    if (this.#active) return;
    if (!isRecord(this.app) || !isRecord(this.app.canvas))
      throw new TypeError('Comfy canvas history is unavailable.');
    this.#active = true;
    const canvas = this.app.canvas;
    this.#bind(canvas, 'onBeforeChange', 'onAfterChange');
    if (
      typeof canvas.emitBeforeChange === 'function' &&
      typeof canvas.emitAfterChange === 'function'
    )
      this.#bind(canvas, 'emitBeforeChange', 'emitAfterChange');
    this.document.defaultView?.addEventListener('pagehide', this.#dispose);
  }

  /** Keep graph hydration inside the host load rather than recording another user edit. */
  setConfiguring(configuring: boolean): void {
    this.#configuring = configuring;
  }

  /** Restore only callbacks still owned by this adapter and balance outstanding native entries. */
  dispose(): void {
    if (!this.#active) return;
    this.#active = false;
    for (const dispose of this.#bindings.splice(0)) dispose();
    this.document.defaultView?.removeEventListener('pagehide', this.#dispose);
  }

  /** Bridge both native transaction vocabularies without replacing their event or callback chains. */
  #bind(canvas: UnknownRecord, beforeName: string, afterName: string): void {
    const originalBefore = canvas[beforeName];
    const originalAfter = canvas[afterName];
    const pending: Array<HistoryEntry | null> = [];
    /** Enter the exact current tracker before invoking native transaction notifications. */
    const before = (...args: unknown[]): unknown => {
      const entry = this.#active && !this.#configuring ? this.#entry() : null;
      pending.push(entry);
      try {
        return typeof originalBefore === 'function'
          ? originalBefore.apply(canvas, args)
          : undefined;
      } catch (error: unknown) {
        pending.pop();
        entry?.finish();
        throw error;
      }
    };
    /** Capture only after the original notification and all nested edits have completed. */
    const after = (...args: unknown[]): unknown => {
      const entry = pending.pop();
      try {
        return typeof originalAfter === 'function' ? originalAfter.apply(canvas, args) : undefined;
      } finally {
        try {
          if (entry) this.beforeCommit();
        } finally {
          entry?.finish();
        }
      }
    };
    canvas[beforeName] = before;
    canvas[afterName] = after;
    this.#bindings.push(() => {
      if (canvas[beforeName] === before) canvas[beforeName] = originalBefore;
      if (canvas[afterName] === after) canvas[afterName] = originalAfter;
      for (const entry of pending.splice(0)) entry?.finish();
    });
  }

  /** Validate the host tracker without copying its state, queue, or undo policy. */
  #entry(): HistoryEntry | null {
    const manager =
      isRecord(this.app) && isRecord(this.app.extensionManager) ? this.app.extensionManager : null;
    const store = manager && isRecord(manager.workflow) ? manager.workflow : null;
    const workflow = store?.activeWorkflow;
    if (!isRecord(workflow)) return null;
    const tracker = workflow.changeTracker;
    if (
      !isRecord(tracker) ||
      typeof tracker.beforeChange !== 'function' ||
      typeof tracker.afterChange !== 'function'
    )
      throw new TypeError('Comfy workflow transaction history is unavailable.');
    if (tracker._restoringState === true) return null;
    const after = tracker.afterChange;
    tracker.beforeChange();
    return {
      finish: () => {
        after.call(tracker);
      },
    };
  }

  /** Release the bridge with its document lifetime. */
  readonly #dispose = (): void => this.dispose();
}
