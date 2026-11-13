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
/** Route wheel gestures through native overflow without per-feature listeners or discovery. */
import { scrollWheelDistance } from './ScrollWheelDistance.js';

/** Give horizontal scroll surfaces consistent input independently of theme and renderer. */
export class ScrollSurfaceWheelHost {
  readonly #target: Window | Document;
  #active = false;

  /** Bind capture to the document's window before graph and widget wheel handlers. */
  constructor(private readonly document: Document) {
    this.#target = document.defaultView ?? document;
  }

  /** Install one delegated listener for current and future light and open-shadow surfaces. */
  setup(): void {
    if (this.#active) return;
    this.#active = true;
    this.#target.addEventListener('wheel', this.#wheel, { capture: true, passive: false });
    this.#target.addEventListener('pagehide', this.dispose);
  }

  /** Release input ownership when the host document is disposed. */
  readonly dispose = (): void => {
    this.#active = false;
    this.#target.removeEventListener('wheel', this.#wheel, true);
    this.#target.removeEventListener('pagehide', this.dispose);
  };

  /** Preserve nested vertical scrolling and zoom while owning gestures inside horizontal surfaces. */
  readonly #wheel = (event: Event): void => {
    if (!(event instanceof WheelEvent) || event.ctrlKey || event.metaKey) return;
    const path = event.composedPath();
    if (
      path.some((node) => node instanceof HTMLElement && node.getAttribute('role') === 'scrollbar')
    )
      return;
    for (const node of path) {
      if (!(node instanceof HTMLElement)) continue;
      const maximum = node.scrollWidth - node.clientWidth;
      const verticalRange = node.scrollHeight - node.clientHeight;
      if (maximum <= 1 && verticalRange <= 1) continue;
      const style = this.document.defaultView?.getComputedStyle(node);
      const horizontal =
        maximum > 1 && /auto|scroll/.test(style?.overflowX || style?.overflow || '');
      const vertical =
        verticalRange > 1 && /auto|scroll/.test(style?.overflowY || style?.overflow || '');
      if (vertical && event.deltaY !== 0 && !event.shiftKey) return;
      if (!horizontal) continue;
      const distance = scrollWheelDistance(event, 'horizontal', node.clientWidth);
      if (!distance) return;
      const rtl = style?.direction === 'rtl';
      const direction = rtl && !event.deltaX ? -1 : 1;
      node.scrollLeft = Math.min(
        rtl ? 0 : maximum,
        Math.max(rtl ? -maximum : 0, node.scrollLeft + distance * direction),
      );
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }
  };
}
