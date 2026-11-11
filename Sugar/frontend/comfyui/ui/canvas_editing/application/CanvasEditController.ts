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
/** Drive one canvas's editing tools from host state and the editing chrome's own intents. */
import type { CanvasEditHost } from '../../../../../integrations/contracts/CanvasEditing.js';
import type { CanvasChangeSubscription } from '../../../../../integrations/contracts/CanvasChangeObservation.js';
import type {
  CanvasBrushOptions,
  CanvasEditCommand,
  CanvasEditState,
  CanvasToolId,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import {
  DEFAULT_COVERAGE_MODIFICATION,
  type CanvasCoverageModification,
} from '../domain/CanvasCoverageModification.js';
import { canvasEditKeyIntent, type CanvasEditKey } from '../domain/CanvasEditKeys.js';
import { canvasEditSurface, type CanvasEditSurface } from '../domain/CanvasEditSurface.js';
import {
  canvasEditSlotOf,
  canvasEditSlotViews,
  canvasEditToolAvailable,
  type CanvasEditSlotId,
  type CanvasEditSlotView,
} from '../domain/CanvasEditTools.js';
import { CanvasEditCommandQueue } from './CanvasEditCommandQueue.js';

/** One immutable view of editing state for the chrome to render. */
export interface CanvasEditSnapshot {
  readonly state: CanvasEditState;
  readonly surface: CanvasEditSurface;
  readonly slots: readonly CanvasEditSlotView[];
  readonly modify: CanvasCoverageModification;
  readonly coverage: CanvasCoverageModification;
}

export interface CanvasEditControllerOptions {
  readonly host: CanvasEditHost;
  report(error: unknown, command: CanvasEditCommand | null): void;
}

/**
 * Own transient editing-chrome state while the host owns the document.
 *
 * The controller renders only from host states, newest revision first, and keeps just what the
 * chrome itself decides: the remembered member of each tool group, the modification amounts being
 * previewed, and whether a selection modification page is open. Nothing is shown until the host
 * presents an editable document.
 */
export class CanvasEditController {
  readonly host: CanvasEditHost;
  readonly #queue: CanvasEditCommandQueue;
  readonly #listeners = new Set<() => void>();
  readonly #remembered = new Map<CanvasEditSlotId, CanvasToolId>();
  #subscription: CanvasChangeSubscription | null = null;
  #state: CanvasEditState | null = null;
  #snapshot: CanvasEditSnapshot | null = null;
  #modifying = false;
  #modify: CanvasCoverageModification = DEFAULT_COVERAGE_MODIFICATION;
  #coverage: CanvasCoverageModification = DEFAULT_COVERAGE_MODIFICATION;
  #navigationHeld = false;
  #disposed = false;

  constructor(private readonly options: CanvasEditControllerOptions) {
    this.host = options.host;
    this.#queue = new CanvasEditCommandQueue(options.host, {
      adopt: (state) => this.#adopt(state),
      report: (error, command) => options.report(error, command),
    });
  }

  /** Observe pushed states and request the current one, since the host pushes only changes. */
  start(): void {
    if (this.#disposed || this.#subscription) return;
    this.#subscription = this.host.changes.subscribe((state) => this.#adopt(state));
    void this.#subscription.ready.then((status) => {
      if (status !== 'observing' && !this.#disposed)
        this.options.report(new Error(`Canvas editing observation is ${status}.`), null);
    });
    void this.#queue.send({ type: 'state' });
  }

  /** Read the latest snapshot; null until the host presents an editable document. */
  read(): CanvasEditSnapshot | null {
    return this.#snapshot;
  }

  /** Notify a listener after every snapshot change; returns its release. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Activate the member a rail slot presents. */
  selectSlot(slot: CanvasEditSlotId): void {
    const view = this.#snapshot?.slots.find((candidate) => candidate.id === slot);
    if (view?.current.available) this.selectTool(view.current.id);
  }

  /** Activate a tool and remember it as its group's presented member. */
  selectTool(tool: CanvasToolId): void {
    const state = this.#state;
    if (!state || !canvasEditToolAvailable(state, tool)) return;
    const slot = canvasEditSlotOf(tool);
    if (slot) this.#remembered.set(slot, tool);
    this.#publish();
    void this.#queue.send({ type: 'tool', tool });
  }

  /** Replace brush or eraser options; drags send only their latest value. */
  setBrushOptions(tool: 'brush' | 'eraser', options: CanvasBrushOptions): void {
    void this.#queue.send({ type: 'options', tool, options }, `options:${tool}`);
  }

  /** Send a one-shot command from a toolbar button. */
  execute(command: CanvasEditCommand): void {
    void this.#queue.send(command);
  }

  /** Open the selection modification page and preview its initial amount at once. */
  beginModify(): void {
    if (!this.#state?.selection || this.#modifying) return;
    this.#modifying = true;
    this.#modify = DEFAULT_COVERAGE_MODIFICATION;
    this.#publish();
    this.#previewModify();
  }

  /** Replace the selection preview from its original with a new operation or amount. */
  updateModify(value: CanvasCoverageModification): void {
    if (!this.#modifying) return;
    this.#modify = value;
    this.#publish();
    this.#previewModify();
  }

  /** Commit the latest selection preview once. */
  async applyModify(): Promise<void> {
    if (!this.#modifying) return;
    const result = await this.#queue.send(this.#modifyCommand('apply'));
    if (result?.kind === 'state') this.#closeModify();
  }

  /** Restore the original selection and close the page. */
  cancelModify(): void {
    if (!this.#modifying && this.#state?.session?.kind !== 'modify') return;
    void this.#queue.send(this.#modifyCommand('cancel'));
    this.#closeModify();
  }

  /** Enter the exclusive whole-mask coverage editor and preview its initial amount. */
  async openCoverage(): Promise<void> {
    if (!this.#state?.activeMask || this.#state.mode === 'coverage') return;
    this.#coverage = DEFAULT_COVERAGE_MODIFICATION;
    const result = await this.#queue.send({ type: 'mode', mode: 'coverage' });
    if (result?.kind === 'state' && result.state.mode === 'coverage') this.#previewCoverage();
  }

  /** Replace the whole-mask preview with a new operation or amount. */
  updateCoverage(value: CanvasCoverageModification): void {
    if (this.#state?.mode !== 'coverage') return;
    this.#coverage = value;
    this.#publish();
    this.#previewCoverage();
  }

  /** Commit the whole-mask preview, then leave the exclusive editor. */
  async applyCoverage(): Promise<void> {
    if (this.#state?.mode !== 'coverage') return;
    const result = await this.#queue.send(this.#coverageCommand('apply'));
    if (result?.kind === 'state') void this.#queue.send({ type: 'mode', mode: 'none' });
  }

  /** Discard the whole-mask preview and leave the exclusive editor. */
  closeCoverage(): void {
    if (this.#state?.mode !== 'coverage') return;
    void this.#queue.send(this.#coverageCommand('cancel'));
    void this.#queue.send({ type: 'mode', mode: 'none' });
  }

  /** Apply one keyboard shortcut; returns whether the canvas consumed the key. */
  key(key: CanvasEditKey): boolean {
    const snapshot = this.#snapshot;
    if (!snapshot) return false;
    const intent = canvasEditKeyIntent(key, snapshot.state, snapshot.surface);
    if (!intent) return false;
    if (intent.kind === 'close-coverage') this.closeCoverage();
    else if (intent.kind === 'cancel-modify') this.cancelModify();
    else if (intent.kind === 'apply-modify') void this.applyModify();
    else if (intent.command.type === 'navigation') this.#navigate(intent.command.held);
    else this.execute(intent.command);
    return true;
  }

  /** Release temporary navigation when the page loses keyboard focus mid-hold. */
  releaseNavigation(): void {
    if (this.#navigationHeld) this.#navigate(false);
  }

  /** Stop observation and drop unsent commands. */
  dispose(): void {
    if (this.#disposed) return;
    this.releaseNavigation();
    this.#disposed = true;
    this.#subscription?.stop();
    this.#subscription = null;
    this.#queue.dispose();
    this.#listeners.clear();
  }

  #navigate(held: boolean): void {
    if (held === this.#navigationHeld) return;
    this.#navigationHeld = held;
    this.execute({ type: 'navigation', held });
  }

  #previewModify(): void {
    void this.#queue.send(this.#modifyCommand('preview'), 'modify:selection');
  }

  #previewCoverage(): void {
    void this.#queue.send(this.#coverageCommand('preview'), 'modify:mask');
  }

  #modifyCommand(phase: 'preview' | 'apply' | 'cancel'): CanvasEditCommand {
    return { type: 'modify', target: 'selection', ...this.#modify, phase };
  }

  #coverageCommand(phase: 'preview' | 'apply' | 'cancel'): CanvasEditCommand {
    return { type: 'modify', target: 'mask', ...this.#coverage, phase };
  }

  #closeModify(): void {
    if (!this.#modifying) return;
    this.#modifying = false;
    this.#publish();
  }

  /** Adopt only newer revisions; a resolved modification session closes its page. */
  #adopt(state: CanvasEditState): void {
    if (this.#disposed || (this.#state && state.revision <= this.#state.revision)) return;
    const previous = this.#state;
    this.#state = state;
    if (this.#modifying && previous?.session?.kind === 'modify' && state.session?.kind !== 'modify')
      this.#modifying = false;
    if (this.#modifying && !state.selection && state.session?.kind !== 'modify')
      this.#modifying = false;
    this.#publish();
  }

  #publish(): void {
    const state = this.#state;
    this.#snapshot =
      state && state.document !== null
        ? Object.freeze({
            state,
            surface: canvasEditSurface(state, this.#modifying),
            slots: canvasEditSlotViews(state, this.#remembered),
            modify: this.#modify,
            coverage: this.#coverage,
          })
        : null;
    for (const listener of [...this.#listeners]) listener();
  }
}
