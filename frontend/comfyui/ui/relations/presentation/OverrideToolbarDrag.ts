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
/** Route grip gestures and keyboard placement without owning coordinates or dock state. */
import type { OverrideToolbarPlacement } from './OverrideToolbarPlacement.js';

/** Keep pointer capture outside native widget gestures, including reparenting during undocking. */
export class OverrideToolbarDrag {
  #pointer: number | null = null;

  /** Bind accessible movement to the authoritative placement owner. */
  constructor(
    private readonly root: HTMLElement,
    private readonly handle: HTMLButtonElement,
    private readonly placement: OverrideToolbarPlacement,
    private readonly begin: () => void,
  ) {
    handle.addEventListener('pointerdown', this.#down);
    handle.addEventListener('pointermove', this.#move);
    handle.addEventListener('pointerup', this.#up);
    handle.addEventListener('pointercancel', this.#up);
    handle.addEventListener('lostpointercapture', this.#up);
    handle.addEventListener('keydown', this.#key);
  }

  /** Release capture and listeners without changing widget or workflow state. */
  dispose(): void {
    this.#release();
    this.handle.removeEventListener('pointerdown', this.#down);
    this.handle.removeEventListener('pointermove', this.#move);
    this.handle.removeEventListener('pointerup', this.#up);
    this.handle.removeEventListener('pointercancel', this.#up);
    this.handle.removeEventListener('lostpointercapture', this.#up);
    this.handle.removeEventListener('keydown', this.#key);
  }

  /** Start primary-pointer gestures on the grip without handing them to the graph. */
  readonly #down = (event: PointerEvent): void => {
    if (event.button !== 0 || this.#pointer !== null) return;
    event.preventDefault();
    event.stopPropagation();
    this.begin();
    this.#pointer = event.pointerId;
    this.root.dataset.scOverrideDragging = '';
    this.placement.begin(event.clientX, event.clientY);
    this.handle.setPointerCapture?.(event.pointerId);
  };

  /** Restore capture after a docked toolbar moves back into its floating host. */
  readonly #move = (event: PointerEvent): void => {
    if (event.pointerId !== this.#pointer) return;
    event.preventDefault();
    event.stopPropagation();
    this.placement.move(event.clientX, event.clientY);
    if (!this.handle.hasPointerCapture?.(event.pointerId))
      this.handle.setPointerCapture?.(event.pointerId);
  };

  /** Complete only the owning pointer and distinguish a drop from cancellation. */
  readonly #up = (event: PointerEvent): void => {
    if (event.pointerId !== this.#pointer) return;
    if (event.type === 'lostpointercapture' && this.handle.hasPointerCapture?.(event.pointerId))
      return;
    event.stopPropagation();
    this.#release();
    this.placement.end(event.clientX, event.clientY, event.type === 'pointerup');
  };

  /** Clear gesture ownership before capture release can emit another completion event. */
  #release(): void {
    const pointer = this.#pointer;
    this.#pointer = null;
    delete this.root.dataset.scOverrideDragging;
    if (pointer !== null && this.handle.hasPointerCapture?.(pointer))
      this.handle.releasePointerCapture(pointer);
  }

  /** Offer keyboard docking, bounded arrow movement, and restoration to the initial floating position. */
  readonly #key = (event: KeyboardEvent): void => {
    const steps: Record<string, readonly [number, number]> = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    const direction = steps[event.key];
    if (!direction && event.key !== 'Home' && event.key !== 'Enter') return;
    event.preventDefault();
    event.stopPropagation();
    this.begin();
    if (event.key === 'Enter') this.placement.toggleDock();
    else if (event.key === 'Home') this.placement.reset();
    else if (direction) {
      const step = event.shiftKey ? 40 : 10;
      this.placement.shift(direction[0] * step, direction[1] * step);
    }
  };
}
