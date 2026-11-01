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
/** Own browser-only canvas resources for documents that select browser presentation. */
import { isRecord } from '../../types/common.js';
import { CanvasImageAllowances } from '../application/CanvasImageAllowances.mjs';
import { BrowserCanvasAttachment } from './BrowserCanvasAttachment.mjs';
import { BrowserCanvasImageLoader } from './BrowserCanvasImageLoader.mjs';
import { BrowserCanvasMediaPosters } from './BrowserCanvasMediaPosters.mjs';
import { BrowserCanvasMediaSessions } from './BrowserCanvasMediaSessions.mjs';
import { BrowserCanvasPosterRenderer } from './BrowserCanvasPosterRenderer.mjs';
import { comfyCanvasMediaUrl, openComfyCanvasMedia } from './ComfyCanvasMedia.js';
import { ComfyCanvasModelRuntime } from './ComfyCanvasModelRuntime.mjs';
/** Retain browser presentation caches independently of shared workflow and controls. */
export class ComfyBrowserCanvasHost {
    document;
    logger;
    #allowances = new CanvasImageAllowances();
    #sessions = new BrowserCanvasMediaSessions();
    #models;
    #posters;
    #window;
    constructor(document, logger) {
        this.document = document;
        this.logger = logger;
        const window = document.defaultView;
        if (!window)
            throw new Error('Output canvas window is unavailable.');
        this.#window = window;
        this.#models = new ComfyCanvasModelRuntime(() => (isRecord(window) ? window.comfyAPI : null));
        this.#posters = new BrowserCanvasMediaPosters(new BrowserCanvasPosterRenderer(document, this.#models));
    }
    /** Mount a browser presenter only after the view's preparation is still current. */
    mount(element, shared) {
        const loader = new BrowserCanvasImageLoader(this.document, {
            create: (blob) => this.#window.URL.createObjectURL(blob),
            revoke: (url) => this.#window.URL.revokeObjectURL(url),
        }, this.#posters);
        return new BrowserCanvasAttachment(element, {
            ...shared,
            allowances: this.#allowances,
            loader,
            logger: this.logger,
            models: this.#models,
            mediaSessions: this.#sessions,
            url: (item) => comfyCanvasMediaUrl(this.document.location.origin, item),
            open: (item) => openComfyCanvasMedia(this.#window, item),
            disposeLoader: () => loader.dispose(),
        });
    }
    /** Retire allowance state when its workflow view leaves the shared registry. */
    remove(view) {
        this.#allowances.remove(view);
    }
    /** Report cached browser posters only at an explicit diagnostics boundary. */
    diagnostics() {
        return this.#posters.diagnostics();
    }
    /** Release browser-only session and poster resources with the owning document. */
    dispose() {
        this.#posters.dispose();
        this.#sessions.dispose();
    }
}
