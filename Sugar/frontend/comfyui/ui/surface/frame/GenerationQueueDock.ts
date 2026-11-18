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
/** Keep Comfy's docked job panel on the right of the frame, with a way to put it away. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { isRecord } from '../../types/common.js';
import { GenerationQueueDockWidth } from './GenerationQueueDockWidth.js';

/** Comfy's docked job panel. */
export const GENERATION_QUEUE_DOCK = '[data-testid="job-history-sidebar"]';
const SIDEBAR_TAB = 'job-history';

/**
 * Present Comfy's docked job panel as the generation queue's pinned sidebar.
 *
 * The panel is Comfy's own side panel, which the frame otherwise keeps hidden together with
 * the icon column that opens and closes it. The frame's styles place the docked panel on the
 * right, where the generation queue has always been pinned; this owner gives it the button that
 * puts it away and the edge that resizes it. Comfy's own choice of side is left alone: changing
 * it rebuilds the workspace.
 */
export class GenerationQueueDock {
  readonly #presence: HostSurfaceObserver;
  readonly #hide: HTMLButtonElement;
  readonly #width: GenerationQueueDockWidth;
  #disposed = false;

  /** Observe only the docked panel's own mount. */
  constructor(
    private readonly document: Document,
    private readonly app: unknown,
    private readonly report: (error: unknown) => void,
    /** The frame's own layer, which holds the resizing edge above Comfy's panels. */
    private readonly layer: HTMLElement = document.body,
  ) {
    this.#hide = document.createElement('button');
    this.#hide.type = 'button';
    this.#hide.className = 'sc-generation-queue-dock-hide';
    this.#hide.setAttribute('aria-label', 'Hide generation queue');
    this.#hide.title = 'Hide generation queue';
    this.#hide.addEventListener('click', this.#close);
    this.#width = new GenerationQueueDockWidth(
      document,
      () => document.defaultView?.localStorage ?? null,
      report,
    );
    this.#presence = new HostSurfaceObserver(document.body, GENERATION_QUEUE_DOCK, () =>
      this.#reconcile(),
    );
    this.#reconcile();
  }

  /** Remove the owned button and its listener. */
  dispose(): void {
    this.#disposed = true;
    this.#presence.disconnect();
    this.#hide.removeEventListener('click', this.#close);
    this.#hide.remove();
    this.#width.dispose();
  }

  /** Keep the hide button and the resizing edge in the docked panel through Comfy's re-renders. */
  #reconcile(): void {
    if (this.#disposed) return;
    const panel = this.document.querySelector<HTMLElement>(GENERATION_QUEUE_DOCK);
    if (!panel) {
      this.#hide.remove();
      this.#width.divider.remove();
      return;
    }
    // The edge lives in the frame's own layer, over the panel's left side: Comfy re-renders its
    // panel and would drop an element placed inside it.
    const side = panel.closest<HTMLElement>('.side-bar-panel') ?? panel;
    this.#width.divider.style.top = `${String(Math.max(0, side.getBoundingClientRect().top))}px`;
    if (this.#width.divider.parentElement !== this.layer) this.layer.append(this.#width.divider);
    const actions =
      panel.querySelector<HTMLElement>('.p-toolbar-start > .ml-auto') ??
      panel.querySelector<HTMLElement>('.p-toolbar-start');
    if (actions && this.#hide.parentElement !== actions) actions.append(this.#hide);
  }

  /** Close the panel through the same owner Comfy's own icon column uses. */
  readonly #close = (): void => {
    const tabs = this.#store('sidebarTab');
    if (isRecord(tabs) && tabs.activeSidebarTabId === SIDEBAR_TAB)
      this.#call(tabs, 'toggleSidebarTab', SIDEBAR_TAB);
  };

  /** Narrow one of Comfy's stores on each use; they are replaced as Comfy starts. */
  #store(name: string): unknown {
    const manager = isRecord(this.app) ? this.app.extensionManager : null;
    return isRecord(manager) ? manager[name] : null;
  }

  /** Call one store method, reporting a failure instead of leaving the panel half moved. */
  #call(store: unknown, method: string, ...values: unknown[]): unknown {
    const target = isRecord(store) ? store[method] : null;
    if (typeof target !== 'function') return undefined;
    try {
      const result: unknown = Reflect.apply(target, store, values);
      if (result instanceof Promise) result.catch((error: unknown) => this.report(error));
      return result;
    } catch (error: unknown) {
      this.report(error);
      return undefined;
    }
  }
}
