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
/** Own asynchronous image loading independently from canvas composition. */
export class CubeCanvasPreviewImageCache {
    #createImage;
    #findLoadedImage;
    #invalidate;
    #logger;
    #entries = new Map();
    /** Bind the browser image boundary and repaint callback. */
    constructor(options) {
        this.#createImage = options.createImage;
        this.#findLoadedImage = options.findLoadedImage ?? null;
        this.#invalidate = options.invalidate;
        this.#logger = options.logger ?? null;
    }
    /** Return a decoded image or begin loading it exactly once. */
    get(url, sourceLocator) {
        const cacheKey = `${sourceLocator ?? ''}\u0000${url}`;
        const loaded = this.#findLoadedImage?.(url, sourceLocator) ?? null;
        if (loaded) {
            this.#entries.set(cacheKey, { image: loaded, ready: true });
            return loaded;
        }
        const cached = this.#entries.get(cacheKey);
        if (cached)
            return cached.ready ? cached.image : null;
        const image = this.#createImage();
        const entry = { image, ready: false };
        this.#entries.set(cacheKey, entry);
        image.onload = () => {
            entry.ready = image.naturalWidth > 0 && image.naturalHeight > 0;
            this.#invalidate();
        };
        image.onerror = () => {
            this.#entries.delete(cacheKey);
            this.#logger?.warn('SugarCubes could not load a Cube preview image.', { url });
            this.#invalidate();
        };
        this.#logger?.debug(`SugarCubes loading Cube preview ${sourceLocator ?? '<unlocated>'} from ${url}`);
        image.src = url;
        if (image.complete && image.naturalWidth > 0 && image.naturalHeight > 0) {
            entry.ready = true;
            return image;
        }
        return null;
    }
}
