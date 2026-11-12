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
/** Coalesce settled display demand without reading graph state or capturing pixels during layout. */
import { inputPreviewResolution } from '../domain/InputPreviewResolution.js';
/** Observe physical content size and retain the highest useful resolution during one preview lifetime. */
export class InputPreviewResolutionObserver {
    frame;
    changed;
    #observer;
    #window;
    #timer = null;
    #resolution;
    #pending = 0;
    #disposed = false;
    /** Use native device-pixel observations where supported and CSS size with DPR as a fallback. */
    constructor(frame, changed) {
        this.frame = frame;
        this.changed = changed;
        this.#window = frame.ownerDocument.defaultView;
        const Observer = frame.ownerDocument.defaultView?.ResizeObserver;
        this.#resolution = Observer ? 0 : 256;
        this.#observer = Observer
            ? new Observer((entries) => {
                const entry = entries.find((value) => value.target === frame);
                if (!entry)
                    return;
                const physical = entry.devicePixelContentBoxSize?.[0];
                // Honor CSS density when the browser scales its viewport independently of the native device box.
                const density = this.#window?.devicePixelRatio ?? 1;
                this.#queue(inputPreviewResolution(Math.max(physical?.inlineSize ?? 0, entry.contentRect.width * density), Math.max(physical?.blockSize ?? 0, entry.contentRect.height * density), 1));
            })
            : null;
        if (this.#observer) {
            try {
                this.#observer.observe(frame, { box: 'device-pixel-content-box' });
            }
            catch (error) {
                // Older browser engines accept only content-box; the same observer still supplies CSS geometry.
                if (!(error instanceof TypeError) && !(error instanceof DOMException))
                    throw error;
                this.#observer.observe(frame);
            }
        }
        this.#window?.addEventListener('resize', this.#resized);
    }
    /** Expose zero until a visible frame supplies useful display demand. */
    get resolution() {
        return this.#resolution;
    }
    /** Release pending geometry work before its source subscription retires. */
    dispose() {
        this.#disposed = true;
        this.#observer?.disconnect();
        if (this.#timer)
            clearTimeout(this.#timer);
        this.#window?.removeEventListener('resize', this.#resized);
    }
    /** Recheck density when browser zoom or display placement changes. */
    #resized = () => {
        const rect = this.frame.getBoundingClientRect();
        this.#queue(inputPreviewResolution(rect.width, rect.height, this.#window?.devicePixelRatio ?? 1));
    };
    /** Admit only settled upgrades, retaining existing pixels while a resize or source swap is in progress. */
    #queue(resolution) {
        if (this.#disposed || resolution === this.#pending)
            return;
        if (this.#timer)
            clearTimeout(this.#timer);
        this.#timer = null;
        this.#pending = resolution > this.#resolution ? resolution : 0;
        if (!this.#pending)
            return;
        this.#timer = setTimeout(() => {
            this.#timer = null;
            this.#resolution = this.#pending;
            this.#pending = 0;
            this.changed();
        }, 120);
    }
}
