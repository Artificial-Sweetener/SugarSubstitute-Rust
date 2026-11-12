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
/** Route an empty rail's vertical wheel input to its explicitly paired masonry viewport. */
import { scrollWheelDistance } from '../ScrollWheelDistance.js';

/** Preserve native rail scrolling and hand off only when the stack has no vertical overflow. */
export class CubeStackWheelRouting {
  /** Bind the paired surfaces without querying graph topology or unrelated scroll containers. */
  constructor(
    private readonly rail: HTMLElement,
    private readonly masonry: HTMLElement,
  ) {
    rail.addEventListener('wheel', this.#wheel, { capture: true, passive: false });
  }

  /** Release the listener before the rail or masonry is replaced. */
  dispose(): void {
    this.rail.removeEventListener('wheel', this.#wheel, true);
  }

  /** Normalize line/page gestures while preserving zoom, horizontal input and nested overflow. */
  readonly #wheel = (event: WheelEvent): void => {
    if (
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      !event.deltaY ||
      this.rail.scrollHeight - this.rail.clientHeight > 1 ||
      this.masonry.hasAttribute('data-sc-surface-moving')
    )
      return;
    for (const target of event.composedPath()) {
      if (target === this.rail) break;
      if (!(target instanceof HTMLElement)) continue;
      const overflow = target.ownerDocument.defaultView?.getComputedStyle(target).overflowY;
      if (target.scrollHeight - target.clientHeight > 1 && /auto|scroll/.test(overflow ?? ''))
        return;
    }
    this.masonry.scrollTop += scrollWheelDistance(event, 'vertical', this.masonry.clientHeight);
    event.preventDefault();
    event.stopPropagation();
  };
}
