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
/**
 * The settings navigation's accent rail: 3 × 16, centred on the selected item, moving between
 * items with WinUI's squash and stretch as Fluent Widgets' `ScaleSlideAnimation` draws it. For
 * the first third of 600 ms the rail stretches to cover the distance between the two items, then
 * it slides into place and shrinks back.
 */
const LENGTH = 16;
const DURATION_MS = 600;
const STRETCH = 'cubic-bezier(0.9, 0.1, 1, 0.2)';
const SETTLE = 'cubic-bezier(0.1, 0.9, 0.2, 1)';
/** The rail of one navigation list. */
export class NavigationRail {
    element;
    #top = null;
    constructor(document) {
        this.element = document.createElement('span');
        this.element.className = 'sc-sw-rail';
        this.element.hidden = true;
    }
    /** Show the rail beside `item`, moving from the previous item when there was one. */
    place(item) {
        if (!item) {
            this.element.hidden = true;
            this.#top = null;
            return;
        }
        const top = item.offsetTop + (item.offsetHeight - LENGTH) / 2;
        const from = this.#top;
        this.#top = top;
        this.element.hidden = false;
        this.element.style.transform = `translateY(${String(top)}px)`;
        if (from === null || from === top)
            return;
        const view = this.element.ownerDocument.defaultView;
        if (typeof this.element.animate !== 'function' ||
            view?.matchMedia?.('(prefers-reduced-motion: reduce)').matches)
            return;
        const stretched = Math.abs(top - from) + LENGTH;
        const at = (y, height, easing) => ({
            transform: `translateY(${String(y)}px)`,
            height: `${String(height)}px`,
            ...(easing ? { easing } : {}),
        });
        // Moving down, the far end leads and the near end follows; moving up, the reverse.
        const forward = top > from;
        this.element.getAnimations?.().forEach((animation) => animation.cancel());
        this.element.animate([
            at(from, LENGTH, STRETCH),
            { ...at(forward ? from : top, stretched, SETTLE), offset: 1 / 3 },
            at(top, LENGTH),
        ], { duration: DURATION_MS });
    }
}
