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
/** Recover native renderer acquisition without surrendering the user's requested presentation. */
import { ComfyRuntimeAssetUnavailableError } from './ComfyRuntimeAssetRequest.js';
/** Own cancellable backoff only while a native presentation awaits its installed assets. */
export class NativeRendererAcquisition {
    document;
    logger;
    #generation = 0;
    #wake = null;
    /** Bind timers to the owning document and diagnostics to the host's logging boundary. */
    constructor(document, logger = console) {
        this.document = document;
        this.logger = logger;
    }
    /** Retain selection during transport outages; fail closed for unsupported component contracts. */
    async acquire(load) {
        this.cancel();
        const generation = this.#generation;
        let attempt = 0;
        while (generation === this.#generation) {
            try {
                const renderer = await load();
                return generation === this.#generation ? renderer : null;
            }
            catch (error) {
                if (generation !== this.#generation)
                    return null;
                if (!(error instanceof ComfyRuntimeAssetUnavailableError))
                    throw error;
                if (attempt === 0)
                    this.logger.warn('SugarCubes native controls await an unavailable ComfyUI asset; acquisition will retry.', { error, url: error.url });
                const windowRef = this.document.defaultView;
                if (!windowRef)
                    return null;
                await new Promise((resolve) => {
                    const timer = windowRef.setTimeout(() => {
                        this.#wake = null;
                        resolve();
                    }, Math.min(500 * 2 ** Math.min(attempt++, 4), 5_000));
                    this.#wake = () => {
                        windowRef.clearTimeout(timer);
                        resolve();
                    };
                });
            }
        }
        return null;
    }
    /** Retire delayed retries and ignore late responses from a departed presentation. */
    cancel() {
        this.#generation += 1;
        this.#wake?.();
        this.#wake = null;
    }
}
