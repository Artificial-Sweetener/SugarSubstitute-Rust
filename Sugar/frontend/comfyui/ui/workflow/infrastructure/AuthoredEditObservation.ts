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
/** Notice when a person has finished an edit anywhere in the page, whichever graph it changed. */

/** The page facilities the observation needs: its document's events and one timer. */
export interface AuthoredEditWindow {
  readonly document: Pick<Document, 'addEventListener' | 'removeEventListener'>;
  setTimeout(handler: () => void, delay: number): number;
  clearTimeout(handle: number): void;
}

/** How long input must rest before the workflow is examined, so typing is never interrupted. */
export const AUTHORED_EDIT_QUIET_MS = 250;

/** Every way a person changes a value: typing, committing a field, releasing the pointer or a key. */
const EDIT_EVENTS = ['input', 'change', 'pointerup', 'keyup'] as const;

/**
 * Report once, after input rests, that the workflow may have been edited.
 *
 * A workflow keeps most of its values inside Cubes, whose graphs raise no change on the
 * workflow's own graph. Input is the one signal every edit shares, so the owner of saving is told
 * about all of them and decides by comparing the workflow itself; an event that changed nothing
 * costs one comparison.
 */
export class AuthoredEditObservation {
  #timer: number | null = null;
  #installed = false;

  /** Bind the page and the owner to tell; nothing is observed until `install`. */
  constructor(
    private readonly window: AuthoredEditWindow,
    private readonly settled: () => void,
  ) {}

  /** Observe input in the capture phase, so a surface that stops propagation is still seen. */
  install(): void {
    if (this.#installed) return;
    this.#installed = true;
    for (const type of EDIT_EVENTS) this.window.document.addEventListener(type, this.#edited, true);
  }

  /** Stop observing and drop a report that has not been made. */
  dispose(): void {
    if (!this.#installed) return;
    this.#installed = false;
    for (const type of EDIT_EVENTS)
      this.window.document.removeEventListener(type, this.#edited, true);
    if (this.#timer !== null) this.window.clearTimeout(this.#timer);
    this.#timer = null;
  }

  /** Restart the quiet period: one report follows the last input of a burst. */
  readonly #edited = (): void => {
    if (this.#timer !== null) this.window.clearTimeout(this.#timer);
    this.#timer = this.window.setTimeout(() => {
      this.#timer = null;
      if (this.#installed) this.settled();
    }, AUTHORED_EDIT_QUIET_MS);
  };
}
