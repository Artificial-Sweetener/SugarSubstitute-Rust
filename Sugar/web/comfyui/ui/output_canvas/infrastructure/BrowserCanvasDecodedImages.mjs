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
/** Keep decoded output images alive across canvas attachments and workflow view changes. */
import { BudgetedMemoryCache } from '../../core/memory/BudgetedMemoryCache.js';
import { canvasImageBytes, } from '../domain/CanvasImageResource.mjs';
/**
 * Own decoded output images for the page lifetime.
 *
 * Every attachment, and therefore every workflow view, borrows images through one shared
 * loader. When a view releases a decoded file or media image it is parked here under its item
 * identity and URL instead of being destroyed, so returning to the view reuses it without
 * another fetch or decode. Displayed images are owned by their attachment and accounted by its
 * admission budget; only parked images are evictable. Live preview frames are never parked.
 */
export class BrowserCanvasDecodedImages {
    loader;
    #parked;
    #disposed = false;
    /** Bind the page-lifetime platform loader and memory allowance. */
    constructor(loader, memory) {
        this.loader = loader;
        this.#parked = new BudgetedMemoryCache({
            id: 'output-canvas-decoded-images',
            registry: memory,
            release: (image) => image.release(),
        });
    }
    /** Create one attachment's port over the shared decoded images. */
    attachment() {
        const pending = new Set();
        const delivered = new Set();
        let disposed = false;
        return {
            start: (input) => {
                if (disposed || this.#disposed)
                    throw new Error('Canvas image loader is disposed.');
                const key = parkingKey(input);
                const parked = key === null ? undefined : this.#parked.take(key);
                if (parked) {
                    return {
                        completion: Promise.resolve(this.#lend(parked, key, delivered)),
                        cancel: () => undefined,
                    };
                }
                const load = this.loader.start(input);
                pending.add(load);
                const completion = load.completion.then((image) => {
                    pending.delete(load);
                    return this.#lend(image, key, delivered);
                }, (error) => {
                    pending.delete(load);
                    throw error;
                });
                return {
                    completion,
                    cancel: () => {
                        pending.delete(load);
                        load.cancel();
                    },
                };
            },
            dispose: () => {
                if (disposed)
                    return;
                disposed = true;
                for (const load of [...pending])
                    load.cancel();
                pending.clear();
                for (const release of [...delivered])
                    release();
            },
        };
    }
    /** Report parked images for explicit diagnostics. */
    diagnostics() {
        return { parkedImages: this.#parked.size, parkedBytes: this.#parked.bytes() };
    }
    /** Release every parked and loading image with the owning document. */
    dispose() {
        this.#disposed = true;
        this.#parked.dispose();
        this.loader.dispose();
    }
    /** Hand one decoded image to an attachment whose release parks reusable images. */
    #lend(image, key, delivered) {
        let released = false;
        const release = () => {
            if (released)
                return;
            released = true;
            delivered.delete(release);
            if (key === null || this.#disposed) {
                image.release();
                return;
            }
            image.handle.remove();
            this.#parked.set(key, image, { tier: 2, bytes: canvasImageBytes(image.dimensions) });
        };
        delivered.add(release);
        return Object.freeze({ handle: image.handle, dimensions: image.dimensions, release });
    }
}
/** Identify reusable decoded content; transient live preview frames have no durable identity. */
function parkingKey(input) {
    return input.payload.kind === 'preview'
        ? null
        : JSON.stringify([input.key, input.payload.kind, input.payload.url]);
}
