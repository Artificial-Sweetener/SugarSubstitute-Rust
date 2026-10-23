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
/** Bind one generation-strip lifetime to the public Comfy controls and frame mount. */
import { ComfyGenerationAdapter } from './ComfyGenerationAdapter.js';
import { GenerationStripView } from './GenerationStripView.js';
import { generationStripCss } from './GenerationStripStyles.js';
import { ComfyGenerationQueueSurface } from './ComfyGenerationQueueSurface.js';
import { applyOwnedTheme } from '../appearance/ThemeService.js';
/** Coordinate stable strip presentation without absorbing host execution or styling policy. */
export class GenerationStripHost {
    frame;
    #adapter;
    #view;
    #style;
    #stop;
    #queue;
    /** Mount source-matched generation controls only for the enabled Substitute frame. */
    constructor(document, frame, app, report) {
        this.frame = frame;
        this.#adapter = new ComfyGenerationAdapter(document, app);
        this.#style = document.createElement('style');
        this.#style.textContent = generationStripCss;
        document.head.append(this.#style);
        this.#view = new GenerationStripView(document, {
            setBatch: (value) => {
                this.#adapter.setBatch(value);
                this.#refresh();
            },
            generate: () => this.#adapter.generate(),
            cancel: () => this.#adapter.cancel(),
            skip: () => this.#adapter.skip(),
            report,
        });
        this.#view.element.setAttribute('data-sc-style-theme-root', '');
        frame.setAttribute('data-generation-strip', '');
        frame.append(this.#view.element);
        this.#queue = new ComfyGenerationQueueSurface(document, frame, this.#view.queueSlot);
        this.#stop = this.#adapter.subscribe(() => this.#refresh());
        this.#refresh();
    }
    /** Restore native visibility and release all execution-surface and settings subscriptions. */
    dispose() {
        this.#queue.dispose();
        this.#stop();
        this.#adapter.dispose();
        this.#view.dispose();
        this.#style.remove();
        this.frame.removeAttribute('data-generation-strip');
    }
    /** Expose the retained generation surface to the frame's placement owner. */
    get element() {
        return this.#view.element;
    }
    /** Retain the queue's native ownership while its visual slot moves between frame and toolbar. */
    refreshPlacement() {
        this.#queue.refreshPosition();
    }
    /** Project the frame's accepted theme onto its retained native queue surface. */
    setPalette(dark) {
        applyOwnedTheme(this.#view.element, dark);
        this.#queue.setPalette(dark);
    }
    /** Project validated host state onto the existing presentation controls. */
    #refresh() {
        this.#view.update(this.#adapter.read());
    }
}
