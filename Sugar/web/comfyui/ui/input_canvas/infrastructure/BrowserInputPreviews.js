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
/** Read bounded image sources and effective masks through Comfy's native codecs. */
import { INPUT_PREVIEW_MAXIMUM } from '../domain/InputPreviewResolution.js';
import { InputPreviewWorkCache } from '../application/InputPreviewWorkCache.js';
import { maskSourceImage } from '../domain/MaskSourcePixels.js';
/** Keep original image decoding, alpha absence and orientation semantics with the native file loader. */
export class BrowserInputPreviews {
    stage;
    rasters;
    #decoded;
    #sources = new WeakMap();
    /** Bind image decoding to the owning document and inject same-origin transport for verification. */
    constructor(stage, rasters, memory) {
        this.stage = stage;
        this.rasters = rasters;
        this.#decoded = new InputPreviewWorkCache(memory);
    }
    /** Stage each full-resolution source once for custom loaders before native interpretation and downsampling. */
    async interpret(source, settings, maximumSize) {
        let staged = this.#sources.get(source);
        if (!staged) {
            staged = this.stage(maskSourceImage(source));
            this.#sources.set(source, staged);
            void staged.catch(() => {
                this.#sources.delete(source);
            });
        }
        return await this.load(await staged, settings, maximumSize);
    }
    /** Decode the actual loader output, retaining native orientation and empty-mask geometry. */
    load(file, settings, maximumSize) {
        if (!Number.isSafeInteger(maximumSize) ||
            maximumSize < 0 ||
            maximumSize > INPUT_PREVIEW_MAXIMUM)
            return Promise.reject(new Error('Invalid source preview size.'));
        const key = JSON.stringify([file, settings, maximumSize]);
        return this.#decoded.read(key, () => this.rasters.load(file, settings, maximumSize), maximumSize === 0);
    }
}
