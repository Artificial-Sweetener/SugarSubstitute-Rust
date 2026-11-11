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
/** Measure owned title fonts once and keep optical alignment current across font loading. */
import { cubeCapLineOffset } from './CubeTextOpticalMetrics.js';
const documents = new WeakMap();
/** Schedule cap-height alignment after an owned title inherits its mounted font. */
export function alignCubeTitleOptically(title) {
    let alignment = documents.get(title.ownerDocument);
    if (!alignment) {
        alignment = new CubeTitleFontAlignment(title.ownerDocument);
        documents.set(title.ownerDocument, alignment);
    }
    alignment.track(title);
}
/** Remeasure mounted titles after a named appearance change updates their inherited fonts. */
export function refreshCubeTitleOpticalAlignment(documentRef) {
    documents.get(documentRef)?.refresh();
}
/** Retain weak title references and invalidate only font measurements when fonts finish loading. */
class CubeTitleFontAlignment {
    #document;
    #titles = new Set();
    #offsets = new Map();
    #context = null;
    #queued = false;
    /** Observe this document's font lifetime without observing graph or presentation mutations. */
    constructor(documentRef) {
        this.#document = documentRef;
        documentRef.fonts?.addEventListener('loadingdone', () => {
            this.#offsets.clear();
            this.#schedule();
        });
    }
    /** Include one newly created title in the next mounted measurement batch. */
    track(title) {
        this.#titles.add(new WeakRef(title));
        this.#schedule();
    }
    /** Reuse measured font metrics while following the current font of every mounted title. */
    refresh() {
        this.#schedule();
    }
    /** Coalesce measurements before painting, leaving detached titles collectible. */
    #schedule() {
        if (this.#queued)
            return;
        const view = this.#document.defaultView;
        if (!view?.requestAnimationFrame || typeof CanvasRenderingContext2D === 'undefined')
            return;
        this.#queued = true;
        view.requestAnimationFrame(() => {
            this.#queued = false;
            this.#context ??= this.#document.createElement('canvas').getContext('2d');
            if (!this.#context)
                return;
            for (const reference of this.#titles) {
                const title = reference.deref();
                if (!title?.isConnected) {
                    this.#titles.delete(reference);
                    continue;
                }
                for (const part of title.querySelectorAll('.sugarcubes-model-title__pill-label, .sugarcubes-model-title__name, .sugarcubes-model-title__suffix')) {
                    const style = view.getComputedStyle(part);
                    const font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
                    let offset = this.#offsets.get(font);
                    if (offset === undefined) {
                        this.#context.font = font;
                        this.#context.textBaseline = 'alphabetic';
                        offset = cubeCapLineOffset(this.#context.measureText('H'));
                        this.#offsets.set(font, offset);
                    }
                    const value = `${offset}px`;
                    if (part.style.getPropertyValue('--sc-cap-line-offset') !== value)
                        part.style.setProperty('--sc-cap-line-offset', value);
                }
            }
        });
    }
}
