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
/** Load and retain media used by the Nodes 1.0 Cube preview rail. */
import { BudgetedMemoryCache } from '../core/memory/BudgetedMemoryCache.js';
/**
 * Retain preview images SugarCubes decoded itself for the page lifetime.
 *
 * Output URLs identify immutable files, so a workflow switch back to the same outputs reuses the
 * decoded image. Waiters are held only until their image settles.
 */
export class CubePreviewImageStore {
    #previews;
    /** Join the page memory allowance as decoded-image storage. */
    constructor(registry) {
        this.#previews = new BudgetedMemoryCache({
            id: 'cube-preview-images',
            registry,
            release: (preview) => {
                preview.waiting.clear();
                preview.image.onload = null;
                preview.image.onerror = null;
                preview.image.removeAttribute('src');
            },
        });
    }
    /** Return a retained decoded image without starting a load. */
    ready(key) {
        const stored = this.#previews.has(key) ? this.#previews.get(key) : undefined;
        return stored?.ready ? stored.image : null;
    }
    /** Return a decoded image, or begin loading it exactly once and notify `settled` when done. */
    load(key, url, createImage, settled, logger) {
        const stored = this.#previews.get(key);
        if (stored) {
            if (!stored.ready)
                stored.waiting.add(settled);
            return stored.ready ? stored.image : null;
        }
        logger?.debug(`SugarCubes loading Cube preview from ${url}`);
        const image = createImage();
        const preview = { image, ready: false, waiting: new Set([settled]) };
        const notify = () => {
            const waiting = [...preview.waiting];
            preview.waiting.clear();
            for (const listener of waiting)
                listener();
        };
        image.onload = () => {
            preview.ready = image.naturalWidth > 0 && image.naturalHeight > 0;
            if (preview.ready)
                this.#previews.resize(key, image.naturalWidth * image.naturalHeight * 4);
            notify();
        };
        image.onerror = () => {
            const waiting = [...preview.waiting];
            this.#previews.delete(key);
            logger?.warn('SugarCubes could not load a Cube preview image.', { url });
            for (const listener of waiting)
                listener();
        };
        this.#previews.set(key, preview, { tier: 2, bytes: 0 });
        image.src = url;
        if (image.complete && image.naturalWidth > 0 && image.naturalHeight > 0) {
            preview.ready = true;
            this.#previews.resize(key, image.naturalWidth * image.naturalHeight * 4);
            return image;
        }
        return null;
    }
    /** Report retained previews for diagnostics and lifetime proofs. */
    get size() {
        return this.#previews.size;
    }
    /** Release every retained preview with the owning page. */
    dispose() {
        this.#previews.dispose();
    }
}
/** Resolve one graph host's preview images through Comfy's own decodes or the shared store. */
export class CubeCanvasPreviewImageCache {
    #options;
    // Comfy-owned decoded images belong to this host's graph and must not outlive it.
    #found = new Map();
    /** Bind the shared store, browser image boundary and this host's repaint callback. */
    constructor(options) {
        this.#options = options;
    }
    /**
     * Return a decoded image or begin loading it exactly once.
     *
     * Retained images answer first: searching Comfy's document and graph images is a full scan,
     * so it runs only while neither this host nor the shared store holds a decoded image.
     */
    get(url, sourceLocator) {
        const key = `${sourceLocator ?? ''}\u0000${url}`;
        const retained = this.#found.get(key) ?? this.#options.images.ready(key);
        if (retained)
            return retained;
        const loaded = this.#options.findLoadedImage?.(url, sourceLocator) ?? null;
        if (loaded) {
            this.#found.set(key, loaded);
            return loaded;
        }
        return this.#options.images.load(key, url, this.#options.createImage, this.#options.invalidate, this.#options.logger ?? null);
    }
}
