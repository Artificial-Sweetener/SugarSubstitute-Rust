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
/** Keep pointer, wheel and focus out of one surface for the length of a motion. */

/** Marks a surface whose input is held; no style rule reads it. */
export const SURFACE_INPUT_HELD_ATTRIBUTE = 'data-sc-input-held';

const POINTER_EVENTS = [
  'pointerdown',
  'pointerup',
  'mousedown',
  'mouseup',
  'click',
  'dblclick',
  'auxclick',
  'contextmenu',
  'wheel',
  'dragstart',
  'touchstart',
] as const;

/**
 * Hold one surface's input without making it `inert`.
 *
 * `inert` reaches every descendant's computed style, so setting and clearing it on a surface of
 * several thousand elements costs a full restyle each way. The hold writes no style at all: it
 * refuses pointer and wheel input bound for the surface from the document's capture phase, ahead
 * of any listener inside the surface, and lets focus rest nowhere within it.
 */
export class SurfaceInputHold {
  #element: HTMLElement | null = null;

  /** Whether a surface is currently held. */
  get held(): boolean {
    return this.#element !== null;
  }

  /** Hold `element`, releasing any surface held before it. */
  hold(element: HTMLElement): void {
    if (this.#element === element) return;
    this.release();
    this.#element = element;
    element.setAttribute(SURFACE_INPUT_HELD_ATTRIBUTE, '');
    const documentRef = element.ownerDocument;
    for (const type of POINTER_EVENTS)
      documentRef.addEventListener(type, this.#refuse, { capture: true, passive: false });
    documentRef.addEventListener('focusin', this.#unfocus, true);
    const focused = documentRef.activeElement;
    if (focused instanceof HTMLElement && element.contains(focused)) focused.blur();
  }

  /** Give the surface its input back. */
  release(): void {
    const element = this.#element;
    if (!element) return;
    this.#element = null;
    const documentRef = element.ownerDocument;
    for (const type of POINTER_EVENTS) documentRef.removeEventListener(type, this.#refuse, true);
    documentRef.removeEventListener('focusin', this.#unfocus, true);
    element.removeAttribute(SURFACE_INPUT_HELD_ATTRIBUTE);
  }

  /** Whether `event` is bound for the held surface. */
  #within(event: Event): boolean {
    return event.target instanceof Node && (this.#element?.contains(event.target) ?? false);
  }

  /** Stop an event bound for the held surface before anything inside it hears it. */
  readonly #refuse = (event: Event): void => {
    if (!this.#within(event)) return;
    event.stopImmediatePropagation();
    if (event.cancelable) event.preventDefault();
  };

  /** Send focus away from the held surface as soon as it arrives. */
  readonly #unfocus = (event: Event): void => {
    if (this.#within(event) && event.target instanceof HTMLElement) event.target.blur();
  };
}
