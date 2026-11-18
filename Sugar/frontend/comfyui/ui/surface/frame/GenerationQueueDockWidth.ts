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
/** Let the docked generation queue be resized from its left edge, and remember the width. */

/** The docked queue's width until a person chooses another: the fly-out's width. */
export const GENERATION_QUEUE_DOCK_DEFAULT_WIDTH = 360;
/** The narrowest the docked queue gets: its rows stay readable. */
export const GENERATION_QUEUE_DOCK_MINIMUM_WIDTH = 280;
/** The widest the docked queue gets, however wide the window is. */
export const GENERATION_QUEUE_DOCK_MAXIMUM_WIDTH = 720;
/** The custom property the frame's styles size the docked panel with. */
export const GENERATION_QUEUE_DOCK_WIDTH_PROPERTY = '--sc-generation-queue-dock-width';
const STORAGE_KEY = 'sugarcubes.generationQueueDockWidth';
const KEY_STEP = 16;

/**
 * Own the docked queue's width.
 *
 * The queue is pinned at the right, so dragging its left edge leftwards widens it. The width is
 * kept between fixed bounds and never past six tenths of the window, which leaves the editor and
 * canvas room. It is a per-browser convenience, so it lives in local storage; a browser that
 * refuses storage keeps the width for the session only.
 */
export class GenerationQueueDockWidth {
  /** The separator on the panel's left edge. Its owner mounts it in the docked panel. */
  readonly divider: HTMLDivElement;
  #width = GENERATION_QUEUE_DOCK_DEFAULT_WIDTH;
  #drag: { readonly pointer: number; readonly x: number; readonly width: number } | null = null;

  constructor(
    private readonly document: Document,
    private readonly storage: () => Storage | null,
    private readonly report: (error: unknown) => void,
  ) {
    this.divider = document.createElement('div');
    this.divider.className = 'sc-generation-queue-dock-divider';
    this.divider.tabIndex = 0;
    this.divider.setAttribute('role', 'separator');
    this.divider.setAttribute('aria-orientation', 'vertical');
    this.divider.setAttribute('aria-label', 'Resize generation queue');
    this.divider.addEventListener('pointerdown', this.#begin);
    this.divider.addEventListener('dblclick', this.#reset);
    this.divider.addEventListener('keydown', this.#key);
    this.#apply(this.#stored() ?? GENERATION_QUEUE_DOCK_DEFAULT_WIDTH);
  }

  /** The width in CSS pixels the docked panel currently takes. */
  get width(): number {
    return this.#width;
  }

  /** Remove the separator, its listeners and the width it applied. */
  dispose(): void {
    this.divider.removeEventListener('pointerdown', this.#begin);
    this.#follow(false);
    this.divider.removeEventListener('dblclick', this.#reset);
    this.divider.removeEventListener('keydown', this.#key);
    this.divider.remove();
    this.document.documentElement.style.removeProperty(GENERATION_QUEUE_DOCK_WIDTH_PROPERTY);
  }

  readonly #begin = (event: PointerEvent): void => {
    if (event.button !== 0) return;
    event.preventDefault();
    this.#drag = { pointer: event.pointerId, x: event.clientX, width: this.#width };
    this.#follow(true);
  };

  /**
   * Follow the pointer across the whole page for the length of a drag. The edge is two pixels
   * wide, so the pointer leaves it at once; the page hears the rest of the gesture wherever it
   * goes.
   */
  #follow(dragging: boolean): void {
    const change = dragging ? 'addEventListener' : 'removeEventListener';
    this.document[change]('pointermove', this.#move as EventListener, true);
    this.document[change]('pointerup', this.#end as EventListener, true);
    this.document[change]('pointercancel', this.#end as EventListener, true);
  }

  readonly #move = (event: PointerEvent): void => {
    const drag = this.#drag;
    if (!drag || drag.pointer !== event.pointerId) return;
    this.#apply(drag.width + (drag.x - event.clientX));
  };

  readonly #end = (event: PointerEvent): void => {
    const drag = this.#drag;
    if (!drag || drag.pointer !== event.pointerId) return;
    this.#drag = null;
    this.#follow(false);
    this.#remember();
  };

  readonly #reset = (): void => {
    this.#apply(GENERATION_QUEUE_DOCK_DEFAULT_WIDTH);
    this.#remember();
  };

  readonly #key = (event: KeyboardEvent): void => {
    const step = event.key === 'ArrowLeft' ? KEY_STEP : event.key === 'ArrowRight' ? -KEY_STEP : 0;
    if (!step) return;
    event.preventDefault();
    this.#apply(this.#width + step);
    this.#remember();
  };

  /** Bound a requested width and size the panel with it. */
  #apply(requested: number): void {
    const view = this.document.defaultView?.innerWidth ?? 0;
    const maximum = Math.max(
      GENERATION_QUEUE_DOCK_MINIMUM_WIDTH,
      Math.min(GENERATION_QUEUE_DOCK_MAXIMUM_WIDTH, view > 0 ? Math.floor(view * 0.6) : Infinity),
    );
    this.#width = Math.round(
      Math.min(maximum, Math.max(GENERATION_QUEUE_DOCK_MINIMUM_WIDTH, requested)),
    );
    this.document.documentElement.style.setProperty(
      GENERATION_QUEUE_DOCK_WIDTH_PROPERTY,
      `${String(this.#width)}px`,
    );
    this.divider.setAttribute('aria-valuenow', String(this.#width));
    this.divider.setAttribute('aria-valuemin', String(GENERATION_QUEUE_DOCK_MINIMUM_WIDTH));
    this.divider.setAttribute('aria-valuemax', String(maximum));
  }

  #stored(): number | null {
    try {
      const text = this.storage()?.getItem(STORAGE_KEY) ?? null;
      const width = text === null ? Number.NaN : Number(text);
      return Number.isFinite(width) ? width : null;
    } catch (error: unknown) {
      this.report(error);
      return null;
    }
  }

  #remember(): void {
    try {
      this.storage()?.setItem(STORAGE_KEY, String(this.#width));
    } catch (error: unknown) {
      this.report(error);
    }
  }
}
