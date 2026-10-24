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
import { encodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
import { maskSourceImage } from '../domain/MaskSourcePixels.js';
import { isRecord } from '../../types/common.js';
/** Keep original image decoding, alpha absence and orientation semantics with the native file loader. */
export class BrowserInputPreviews {
    document;
    stage;
    request;
    #decoded = new InputPreviewWorkCache();
    #sources = new WeakMap();
    /** Bind image decoding to the owning document and inject same-origin transport for verification. */
    constructor(document, stage, request = (input, init) => fetch(input, init)) {
        this.document = document;
        this.stage = stage;
        this.request = request;
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
            maximumSize < 1 ||
            maximumSize > INPUT_PREVIEW_MAXIMUM)
            return Promise.reject(new Error('Invalid source preview size.'));
        const key = JSON.stringify([file, settings, maximumSize]);
        return this.#decoded.read(key, () => this.#load(file, settings, maximumSize));
    }
    /** Decode one bounded native-loader response and release its temporary bitmap. */
    async #load(file, settings, maximumSize) {
        const response = await this.request('/sugarcubes/v2/canvas-inputs/mask-preview', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ file, settings, maximumSize }),
            signal: AbortSignal.timeout(30000),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(isRecord(error) && typeof error.error === 'string'
                ? error.error.slice(0, 512)
                : isRecord(error) && isRecord(error.error) && typeof error.error.message === 'string'
                    ? error.error.message.slice(0, 512)
                    : 'The source preview is unavailable.');
        }
        const blob = await response.blob();
        if (blob.type !== 'image/png' || blob.size > 8 * 1024 * 1024)
            throw new Error('The source preview has an invalid image payload.');
        const bitmap = await createImageBitmap(blob);
        try {
            if (bitmap.width < 1 ||
                bitmap.height < 1 ||
                bitmap.width > INPUT_PREVIEW_MAXIMUM ||
                bitmap.height > INPUT_PREVIEW_MAXIMUM)
                throw new Error('The source preview exceeds its size limit.');
            const canvas = this.document.createElement('canvas');
            canvas.width = bitmap.width;
            canvas.height = bitmap.height;
            const context = canvas.getContext('2d');
            if (!context)
                throw new Error('Source preview rendering is unavailable.');
            context.drawImage(bitmap, 0, 0);
            const data = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
            if (settings.kind === 'image')
                return encodeCanvasRaster(bitmap.width, bitmap.height, 'rgba8', new Uint8Array(data));
            const coverage = new Uint8Array(bitmap.width * bitmap.height);
            for (let index = 0; index < coverage.length; index++)
                coverage[index] = data[index * 4] ?? 0;
            canvas.width = canvas.height = 1;
            return encodeCanvasRaster(bitmap.width, bitmap.height, 'coverage8', coverage);
        }
        finally {
            bitmap.close();
        }
    }
}
