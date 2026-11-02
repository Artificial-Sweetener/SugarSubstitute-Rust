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
/** Bound and reuse expensive file decoding independently of browser transport and raster interpretation. */
import { INPUT_PREVIEW_MAXIMUM } from '../domain/InputPreviewResolution.js';
import { canvasRasterByteLength, MAX_CANVAS_RASTER_BYTES, } from '../../core/embedded/CanvasRuntime.js';
import { BudgetedMemoryCache } from '../../core/memory/BudgetedMemoryCache.js';
/**
 * Share pending work and retain at most sixteen rasters within the raster byte bound.
 *
 * One instance serves preview and full-resolution decodes so both share one retained-byte bound
 * and one memory consumer. Preview decodes run two at a time within the preview size limit; a
 * full-resolution decode occupies every decoder slot so two such rasters are never decoded at once.
 */
export class InputPreviewWorkCache {
    concurrency;
    #ready;
    #jobs = new Map();
    #queue = [];
    #active = 0;
    /** Register decoded rasters with the page allowance and bound concurrent decoders. */
    constructor(memory, concurrency = 2) {
        this.concurrency = concurrency;
        this.#ready = new BudgetedMemoryCache({
            id: 'input-canvas-previews',
            registry: memory,
            maxEntries: 16,
            maxBytes: MAX_CANVAS_RASTER_BYTES,
        });
    }
    /** Reuse completed pixels or admit one bounded pending decode for this exact file, settings and size. */
    read(key, load, fullResolution = false) {
        const ready = this.#ready.get(key);
        if (ready)
            return Promise.resolve(ready);
        const pending = this.#jobs.get(key);
        if (pending)
            return pending.promise;
        if (this.#jobs.size >= 128)
            return Promise.reject(new Error('Too many pending source previews.'));
        let resolve = () => { }, reject = () => { };
        const promise = new Promise((success, failure) => {
            resolve = success;
            reject = failure;
        });
        const job = { key, load, fullResolution, promise, resolve, reject };
        this.#jobs.set(key, job);
        this.#queue.push(job);
        this.#drain();
        return promise;
    }
    /** Release decoder slots on both success and failure before admitting the next queued source. */
    #drain() {
        while (this.#queue.length) {
            const job = this.#queue[0];
            if (!job)
                return;
            const weight = this.#weight(job);
            if (this.#active + weight > this.concurrency)
                return;
            this.#queue.shift();
            this.#active += weight;
            void Promise.resolve()
                .then(job.load)
                .then((pixels) => {
                if (!Number.isSafeInteger(pixels.width) ||
                    !Number.isSafeInteger(pixels.height) ||
                    pixels.width < 1 ||
                    pixels.height < 1 ||
                    (!job.fullResolution &&
                        (pixels.width > INPUT_PREVIEW_MAXIMUM || pixels.height > INPUT_PREVIEW_MAXIMUM)))
                    throw new Error('The source preview exceeds its size limit.');
                this.#ready.set(job.key, pixels, {
                    tier: 2,
                    bytes: canvasRasterByteLength(pixels.width, pixels.height, pixels.format),
                });
                return pixels;
            })
                .then(job.resolve, job.reject)
                .finally(() => {
                this.#jobs.delete(job.key);
                this.#active -= weight;
                this.#drain();
            });
        }
    }
    /** Give full-resolution decodes exclusive use of every decoder slot. */
    #weight(job) {
        return job.fullResolution ? this.concurrency : 1;
    }
}
