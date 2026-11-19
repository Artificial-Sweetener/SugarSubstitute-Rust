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
/** Hold the generation queue as a callout that points at the queue button. */
import { FloatingSurface } from '../../../menus/FloatingSurface.js';
import {
  GENERATION_QUEUE_ARROW_HEIGHT,
  GENERATION_QUEUE_FLYOUT_WIDTH,
} from './GenerationQueueViewStyles.js';

/**
 * Own where the queue's fly-out sits and when it leaves.
 *
 * The fly-out hangs centred under the queue button, over the toolbar and the canvas, with an arrow
 * on its top edge pointing back at the button. The shared floating surface keeps it inside the
 * window, so the arrow is placed from where the surface actually landed. A press anywhere outside
 * the fly-out and its button puts it away; the button itself toggles it.
 */
export class GenerationQueueFlyout {
  readonly #surface: FloatingSurface;
  #button: HTMLElement | null = null;

  constructor(private readonly content: HTMLElement) {
    this.#surface = new FloatingSurface(content);
  }

  get open(): boolean {
    return this.#button !== null;
  }

  /** Show the queue under `button`; `closed` is told when a press elsewhere puts it away. */
  show(button: HTMLElement, closed: () => void): void {
    this.hide();
    this.#button = button;
    const bounds = button.getBoundingClientRect();
    const centre = bounds.left + bounds.width / 2;
    const left = centre - GENERATION_QUEUE_FLYOUT_WIDTH / 2;
    this.#surface.open({
      anchor: {
        kind: 'below',
        bounds: {
          left,
          right: left + GENERATION_QUEUE_FLYOUT_WIDTH,
          top: bounds.top,
          // The arrow fills the gap between the button and the surface.
          bottom: bounds.bottom + GENERATION_QUEUE_ARROW_HEIGHT,
        },
      },
      dismiss: () => {
        this.hide();
        closed();
      },
      containsTarget: (target) => button.contains(target),
    });
    const placed = this.content.getBoundingClientRect();
    this.content.style.setProperty('--sc-gq-arrow', `${String(centre - placed.left)}px`);
    button.setAttribute('aria-expanded', 'true');
  }

  /** Put the queue away. */
  hide(): void {
    if (!this.#button) return;
    this.#button.setAttribute('aria-expanded', 'false');
    this.#button = null;
    this.#surface.close();
  }
}
