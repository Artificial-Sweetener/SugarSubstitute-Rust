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
/** Share pending work and retain at most sixteen maximum-size rasters with two concurrent decoders. */
export class InputPreviewWorkCache {
    #ready = new Map();
    #jobs = new Map();
    #queue = [];
    #active = 0;
    /** Reuse completed pixels or admit one bounded pending decode for this exact file, settings and size. */
    read(key, load) {
        const ready = this.#ready.get(key);
        if (ready) {
            this.#ready.delete(key);
            this.#ready.set(key, ready);
            return Promise.resolve(ready);
        }
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
        const job = { key, load, promise, resolve, reject };
        this.#jobs.set(key, job);
        this.#queue.push(job);
        this.#drain();
        return promise;
    }
    /** Release a decoder slot on both success and failure before admitting the next queued source. */
    #drain() {
        while (this.#active < 2 && this.#queue.length) {
            const job = this.#queue.shift();
            if (!job)
                return;
            this.#active++;
            void Promise.resolve()
                .then(job.load)
                .then((pixels) => {
                if (!Number.isSafeInteger(pixels.width) ||
                    !Number.isSafeInteger(pixels.height) ||
                    pixels.width < 1 ||
                    pixels.height < 1 ||
                    pixels.width > INPUT_PREVIEW_MAXIMUM ||
                    pixels.height > INPUT_PREVIEW_MAXIMUM)
                    throw new Error('The source preview exceeds its size limit.');
                this.#ready.set(job.key, pixels);
                while (this.#ready.size > 16)
                    this.#ready.delete(this.#ready.keys().next().value ?? '');
                return pixels;
            })
                .then(job.resolve, job.reject)
                .finally(() => {
                this.#jobs.delete(job.key);
                this.#active--;
                this.#drain();
            });
        }
    }
}
