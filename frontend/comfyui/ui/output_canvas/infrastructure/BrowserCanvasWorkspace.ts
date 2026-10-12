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
/** Mount explicitly allocated canvas/content siblings inside the qualified Comfy workspace. */

import type { CanvasFrameClock } from '../application/CanvasCameraTransition.js';
import type { CanvasWorkspaceLayout } from '../application/CanvasWorkspaceLayout.js';
import type {
  CanvasWorkspaceAllocation,
  CanvasWorkspacePreferences,
} from '../domain/CanvasWorkspace.js';
import { CanvasWorkspaceResizeInput } from '../presentation/browser/CanvasWorkspaceResizeInput.js';
import { ComfyCanvasFrameAllocation } from './ComfyCanvasFrameAllocation.js';
import { ensureBrowserCanvasWorkspaceStyles } from './BrowserCanvasWorkspaceStyles.js';

export interface BrowserCanvasWorkspaceActions {
  /** Persist a completed width through the sole host preferences owner. */
  width(width: number): void;
  /** Request explicit close through the same action used by the native toolbar toggle. */
  close(): void;
  /** Reconcile canvas attachment availability after settled allocation changes. */
  changed(): void;
}

/** Keep graph frame, sibling bounds, and resize input coordinated without owning workflow results. */
export class BrowserCanvasWorkspace {
  readonly content: HTMLDivElement;
  readonly canvas: HTMLDivElement;
  readonly #pane: HTMLElement;
  readonly #backdrop: HTMLButtonElement;
  readonly #divider: HTMLDivElement;
  readonly #close: HTMLButtonElement;
  readonly #frame: ComfyCanvasFrameAllocation;
  readonly #resize: ResizeObserver | null;
  readonly #input: CanvasWorkspaceResizeInput;
  #pending: (() => void) | null = null;
  #allocation: CanvasWorkspaceAllocation | null = null;
  #suspended = false;
  #disposed = false;

  /** Allocate only owned nodes, keeping Comfy's chrome and every native render layer in its original parent. */
  constructor(
    root: HTMLElement,
    private readonly panel: HTMLElement,
    private readonly model: CanvasWorkspaceLayout,
    private readonly frames: CanvasFrameClock,
    private readonly actions: BrowserCanvasWorkspaceActions,
  ) {
    this.#frame = new ComfyCanvasFrameAllocation(root, panel);
    const document = panel.ownerDocument;
    ensureBrowserCanvasWorkspaceStyles(document);
    this.content = document.createElement('div');
    this.content.className = 'sc-output-workspace-content';
    this.#pane = document.createElement('aside');
    this.#pane.className = 'sc-output-workspace-pane';
    this.#pane.setAttribute('aria-label', 'Workflow output canvas');
    this.#pane.setAttribute('data-capture-wheel', 'true');
    this.#close = document.createElement('button');
    this.#close.className = 'sc-output-workspace-close';
    this.#close.type = 'button';
    this.#close.setAttribute('aria-label', 'Close output canvas');
    this.#close.textContent = '×';
    this.#close.addEventListener('click', actions.close);
    this.canvas = document.createElement('div');
    this.canvas.className = 'sc-output-workspace-view';
    this.#divider = document.createElement('div');
    this.#divider.className = 'sc-output-workspace-divider';
    this.#divider.tabIndex = 0;
    this.#divider.setAttribute('role', 'separator');
    this.#divider.setAttribute('aria-label', 'Resize output canvas');
    this.#divider.setAttribute('aria-orientation', 'vertical');
    this.#pane.append(this.canvas, this.#close, this.#divider);
    this.#backdrop = document.createElement('button');
    this.#backdrop.className = 'sc-output-workspace-backdrop';
    this.#backdrop.type = 'button';
    this.#backdrop.setAttribute('aria-label', 'Close output canvas drawer');
    this.#backdrop.addEventListener('click', actions.close);
    panel.append(this.content, this.#backdrop, this.#pane);
    let input: CanvasWorkspaceResizeInput | null = null;
    let observer: ResizeObserver | null = null;
    try {
      this.#input = input = new CanvasWorkspaceResizeInput(this.#divider, frames, {
        width: () => model.read().canvas.width,
        preview: (width) => {
          model.resize(width);
          this.#render();
        },
        commit: actions.width,
        cancel: () => {
          model.cancelResize();
          this.#render();
        },
      });
      const Observer = document.defaultView?.ResizeObserver;
      this.#resize = observer = Observer ? new Observer(this.#schedule) : null;
      this.#resize?.observe(panel);
      document.defaultView?.addEventListener('resize', this.#schedule);
      this.#measure();
    } catch (error: unknown) {
      this.#disposed = true;
      input?.dispose();
      observer?.disconnect();
      this.#pending?.();
      document.defaultView?.removeEventListener('resize', this.#schedule);
      this.#close.removeEventListener('click', actions.close);
      this.#backdrop.removeEventListener('click', actions.close);
      this.#frame.dispose();
      this.#pane.remove();
      this.#backdrop.remove();
      this.content.remove();
      throw error;
    }
  }

  /** Read actual primary-surface availability rather than the stored desktop visibility preference. */
  get visible(): boolean {
    return !this.#disposed && !this.#suspended && this.#allocation?.visible === true;
  }

  /** Apply accepted user settings without moving or remounting existing masonry controls. */
  preferences(preferences: CanvasWorkspacePreferences): void {
    this.model.preferences(preferences);
    this.#render();
  }

  /** Open/close a narrow drawer only after its explicit user action is accepted. */
  drawer(open: boolean): void {
    this.model.drawer(open);
    this.#render();
    if (open && this.#allocation?.mode === 'drawer') this.#close.focus({ preventScroll: true });
  }

  /** Keep content identity while the host temporarily hides graph mode or configures its workflow. */
  suspend(suspended: boolean): void {
    if (this.#suspended === suspended || this.#disposed) return;
    this.#suspended = suspended;
    this.#input.cancel();
    this.model.cancelResize();
    this.#render();
  }

  /** Relinquish only owned regions and restore the native graph's exact frame styles. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#input.dispose();
    this.#pending?.();
    this.#pending = null;
    this.#resize?.disconnect();
    this.panel.ownerDocument.defaultView?.removeEventListener('resize', this.#schedule);
    this.#close.removeEventListener('click', this.actions.close);
    this.#backdrop.removeEventListener('click', this.actions.close);
    this.#frame.dispose();
    this.#pane.remove();
    this.#backdrop.remove();
    this.content.remove();
  }

  /** Coalesce native workspace resize notifications without observing arbitrary presentation mutations. */
  readonly #schedule = (): void => {
    if (this.#pending || this.#disposed) return;
    this.#pending = this.frames.request(() => {
      this.#pending = null;
      this.#measure();
    });
  };

  /** Measure the compensated native workspace, whose bounds remain independent of the reduced graph origin. */
  #measure(): void {
    if (this.#disposed) return;
    const bounds = this.panel.getBoundingClientRect();
    const previous = this.model.read().mode;
    this.model.bounds(bounds.width, bounds.height);
    if (this.model.read().mode !== previous) this.#input.cancel();
    this.#render();
  }

  /** Apply one authoritative rectangle to masonry, its backdrop, rail, and finder through their shared parent. */
  #render(): void {
    if (this.#disposed) return;
    const allocation = this.model.read();
    this.#allocation = allocation;
    const visible = allocation.visible && !this.#suspended;
    const inset = visible && allocation.mode === 'docked' ? allocation.canvas.width : 0;
    this.#frame.apply(inset);
    this.content.style.left = '0px';
    this.content.style.width = `calc(100% - ${inset}px)`;
    this.#pane.style.width = `${allocation.canvas.width}px`;
    this.#pane.hidden = !visible;
    this.#backdrop.hidden = !visible || allocation.mode !== 'drawer';
    this.#backdrop.style.right = `${allocation.canvas.width}px`;
    this.#divider.setAttribute('aria-valuenow', String(Math.round(allocation.canvas.width)));
    this.#divider.setAttribute('aria-valuemin', String(Math.round(allocation.minimumWidth)));
    this.#divider.setAttribute('aria-valuemax', String(Math.round(allocation.maximumWidth)));
    this.actions.changed();
  }
}
