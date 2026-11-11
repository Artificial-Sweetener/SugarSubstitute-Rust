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
/** Own the Nodes 2 pointer lifecycle for Cube output-rail resizing. */
import { clampCubePreviewWidth } from './CubePreviewResizeGeometry.js';
/** Translate one divider drag into a renderer-neutral persisted rail width. */
export class CubePreviewDividerController {
    #element;
    #events;
    #onResize;
    #getScale;
    #range = { minimum: 0, maximum: 0 };
    #width = 0;
    #session = null;
    /** Bind the divider without taking ownership of Cube surface persistence. */
    constructor(options) {
        this.#element = options.element;
        this.#events = options.events;
        this.#getScale = options.getScale ?? (() => 1);
        this.#onResize = options.onResize;
        this.#element.addEventListener('pointerdown', this.#handlePointerDown);
    }
    /** Update current geometry after either renderer or node size changes. */
    setGeometry(width, range, enabled) {
        this.#width = clampCubePreviewWidth(width, range);
        this.#range = range;
        this.#element.hidden = !enabled;
    }
    /** Release every pointer listener owned by this divider. */
    dispose() {
        this.#element.removeEventListener('pointerdown', this.#handlePointerDown);
        this.#detachSession();
    }
    /** Begin one primary-button width transaction. */
    #handlePointerDown = (event) => {
        if (event.button !== 0 || this.#element.hidden)
            return;
        event.preventDefault();
        event.stopPropagation();
        this.#session = {
            pointerId: event.pointerId,
            startX: event.clientX,
            startWidth: this.#width,
        };
        this.#element.setPointerCapture?.(event.pointerId);
        this.#events.addEventListener('pointermove', this.#handlePointerMove, true);
        this.#events.addEventListener('pointerup', this.#handlePointerUp, true);
        this.#events.addEventListener('pointercancel', this.#handlePointerCancel, true);
    };
    /** Apply live width while the divider moves left or right. */
    #handlePointerMove = (event) => {
        const session = this.#session;
        if (!session || event.pointerId !== session.pointerId)
            return;
        event.preventDefault();
        const width = clampCubePreviewWidth(session.startWidth - (event.clientX - session.startX) / this.#scale(), this.#range);
        this.#width = width;
        this.#onResize(width, false);
    };
    /** Commit one completed resize through the Cube surface state owner. */
    #handlePointerUp = (event) => {
        this.#finish(event, true);
    };
    /** End a cancelled pointer without persisting an incomplete width. */
    #handlePointerCancel = (event) => {
        this.#finish(event, false);
    };
    /** Close one matching pointer transaction. */
    #finish(event, committed) {
        const session = this.#session;
        if (!session || event.pointerId !== session.pointerId)
            return;
        event.preventDefault();
        this.#element.releasePointerCapture?.(event.pointerId);
        this.#detachSession();
        if (committed)
            this.#onResize(this.#width, true);
    }
    /** Resolve the current finite host zoom for viewport-to-node conversion. */
    #scale() {
        const scale = this.#getScale();
        return Number.isFinite(scale) && scale > 0 ? scale : 1;
    }
    /** Remove global listeners after either completion or disposal. */
    #detachSession() {
        this.#session = null;
        this.#events.removeEventListener('pointermove', this.#handlePointerMove, true);
        this.#events.removeEventListener('pointerup', this.#handlePointerUp, true);
        this.#events.removeEventListener('pointercancel', this.#handlePointerCancel, true);
    }
}
