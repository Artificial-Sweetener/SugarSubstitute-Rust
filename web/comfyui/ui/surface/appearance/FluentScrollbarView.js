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
/** Present accessible overlay handles that delegate every value change to native scrolling. */
import { scrollbarDragPosition, scrollbarGeometry } from './FluentScrollbarGeometry.js';
import { scrollWheelDistance } from '../ScrollWheelDistance.js';
/** Keep input handling on the owned thumb, outside graph drag and zoom handlers. */
export class FluentScrollbarView {
    element;
    #owner;
    #axis;
    #schedule;
    #geometry = null;
    #drag = null;
    #rtl = false;
    /** Create a layout-neutral control linked to its native scroll owner. */
    constructor(owner, axis, ownerId, schedule) {
        this.#owner = owner;
        this.#axis = axis;
        this.#schedule = schedule;
        this.element = owner.ownerDocument.createElement('div');
        this.element.className = 'sc-fluent-scrollbar';
        this.element.hidden = true;
        this.element.tabIndex = 0;
        this.element.setAttribute('role', 'scrollbar');
        this.element.setAttribute('aria-label', `${axis === 'vertical' ? 'Vertical' : 'Horizontal'} scroll`);
        this.element.setAttribute('aria-orientation', axis);
        this.element.setAttribute('aria-controls', ownerId);
        this.element.setAttribute('aria-valuemin', '0');
        this.element.addEventListener('pointerdown', this.beginDrag);
        this.element.addEventListener('pointermove', this.#pointerMove);
        this.element.addEventListener('pointerup', this.#pointerEnd);
        this.element.addEventListener('pointercancel', this.#pointerEnd);
        this.element.addEventListener('lostpointercapture', this.#pointerEnd);
        this.element.addEventListener('keydown', this.#keyDown);
        this.element.addEventListener('wheel', this.#wheel, { passive: false });
    }
    /** Follow native range and viewport clipping inside the owning surface. */
    update(track, space) {
        const vertical = this.#axis === 'vertical';
        const style = this.#owner.ownerDocument.defaultView?.getComputedStyle(this.#owner);
        const overflow = (vertical ? style?.overflowY : style?.overflowX) || style?.overflow;
        if (this.#owner !== this.#owner.ownerDocument.scrollingElement &&
            !/auto|scroll/.test(overflow ?? '')) {
            this.element.hidden = true;
            return;
        }
        this.#rtl = track?.rtl ?? false;
        const viewport = vertical ? this.#owner.clientHeight : this.#owner.clientWidth;
        const content = vertical ? this.#owner.scrollHeight : this.#owner.scrollWidth;
        const position = this.#position();
        const size = track ? (vertical ? track.height : track.width) - 2 : 0;
        this.#geometry = scrollbarGeometry(size, viewport, content, position);
        const geometry = this.#geometry;
        if (!track || !geometry) {
            this.element.hidden = true;
            return;
        }
        const offset = !vertical && this.#rtl ? geometry.travel - geometry.offset : geometry.offset;
        const left = vertical
            ? this.#rtl
                ? track.left
                : track.left + track.width - 12
            : track.left + 1 + offset;
        const top = vertical ? track.top + 1 + offset : track.horizontalBottom - 12;
        this.element.hidden = false;
        const localLeft = (left - space.left) / space.scaleX;
        const localTop = (top - space.top) / space.scaleY;
        const width = (vertical ? 12 : geometry.length) / space.scaleX;
        const height = (vertical ? geometry.length : 12) / space.scaleY;
        const css = `left:${localLeft}px;top:${localTop}px;width:${width}px;height:${height}px;--sc-scrollbar-scale-x:${space.scaleX};--sc-scrollbar-scale-y:${space.scaleY}`;
        if (this.element.getAttribute('style') !== css)
            this.element.setAttribute('style', css);
        this.element.setAttribute('aria-valuemax', String(Math.round(geometry.maximum)));
        this.element.setAttribute('aria-valuenow', String(Math.round(position)));
    }
    /** Release capture and disconnect the owned control. */
    dispose() {
        this.#pointerEnd();
        this.element.remove();
    }
    /** Suppress a track while its owner is between stable presentation coordinates. */
    hide() {
        this.element.hidden = true;
    }
    /** Read native RTL coordinates as a positive distance from the leading edge. */
    #position() {
        return this.#axis === 'vertical' ? this.#owner.scrollTop : Math.abs(this.#owner.scrollLeft);
    }
    /** Write native state rather than accumulating independent scrollbar state. */
    #scroll(position) {
        if (this.#axis === 'vertical')
            this.#owner.scrollTop = position;
        else
            this.#owner.scrollLeft = this.#rtl ? -position : position;
        this.#schedule();
    }
    /** Capture dragging without sending pointer gestures to the graph canvas. */
    beginDrag = (event) => {
        if (event.button !== 0 || !this.#geometry)
            return;
        event.preventDefault();
        event.stopPropagation();
        this.#drag = {
            pointer: event.pointerId,
            coordinate: this.#axis === 'vertical' ? event.clientY : event.clientX,
            position: this.#position(),
            geometry: this.#geometry,
        };
        this.element.setPointerCapture(event.pointerId);
        this.element.setAttribute('data-dragging', '');
        this.element.focus({ preventScroll: true });
    };
    /** Map captured movement using the range at gesture start. */
    #pointerMove = (event) => {
        if (!this.#drag || event.pointerId !== this.#drag.pointer)
            return;
        event.preventDefault();
        event.stopPropagation();
        const coordinate = this.#axis === 'vertical' ? event.clientY : event.clientX;
        const direction = this.#axis === 'horizontal' && this.#rtl ? -1 : 1;
        this.#scroll(scrollbarDragPosition(this.#drag.geometry, this.#drag.position, (coordinate - this.#drag.coordinate) * direction));
    };
    /** Finish a gesture even when the scroll owner disappears mid-drag. */
    #pointerEnd = () => {
        const drag = this.#drag;
        this.#drag = null;
        if (drag && this.element.hasPointerCapture(drag.pointer))
            this.element.releasePointerCapture(drag.pointer);
        this.element.removeAttribute('data-dragging');
    };
    /** Preserve conventional keyboard scrolling on a focused accessible scrollbar. */
    #keyDown = (event) => {
        if (!this.#geometry)
            return;
        const page = this.#axis === 'vertical' ? this.#owner.clientHeight : this.#owner.clientWidth;
        const values = {
            ArrowDown: 40,
            ArrowUp: -40,
            ArrowRight: this.#rtl ? -40 : 40,
            ArrowLeft: this.#rtl ? 40 : -40,
            PageDown: page,
            PageUp: -page,
        };
        const delta = values[event.key];
        if (delta === undefined && event.key !== 'Home' && event.key !== 'End')
            return;
        event.preventDefault();
        event.stopPropagation();
        this.#scroll(event.key === 'Home'
            ? 0
            : event.key === 'End'
                ? this.#geometry.maximum
                : this.#position() + (delta ?? 0));
    };
    /** Keep wheel gestures over the handle in its scroll area rather than the graph. */
    #wheel = (event) => {
        if (!this.#geometry)
            return;
        event.preventDefault();
        event.stopPropagation();
        const page = this.#axis === 'vertical' ? this.#owner.clientHeight : this.#owner.clientWidth;
        const distance = scrollWheelDistance(event, this.#axis, page);
        this.#scroll(Math.min(this.#geometry.maximum, Math.max(0, this.#position() + distance)));
    };
}
