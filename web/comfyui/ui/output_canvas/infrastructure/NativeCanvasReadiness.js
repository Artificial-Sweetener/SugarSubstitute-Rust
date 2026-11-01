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
/** Bound one native attachment's preparation and settle disposal before its registry lease retires. */
import { CanvasPreparationTimeout } from '../domain/CanvasPreparation.js';
/** Observe real host readiness once, retaining failure for the attachment transaction owner. */
export class NativeCanvasReadiness {
    ready;
    #complete = null;
    #fail = null;
    #stop = null;
    /**
     * Use the existing injected clock and the native host's five-second preparation bound.
     *
     * @param expired Called when the deadline passes, before readiness rejects with a
     *   `CanvasPreparationTimeout` the attachment transaction owner may recover from.
     */
    constructor(clock, expired = () => { }) {
        this.ready = new Promise((resolve, reject) => {
            this.#complete = resolve;
            this.#fail = reject;
        });
        // Constructor failure may prevent the attachment owner from receiving this promise.
        // Keep the original promise rejected for that owner's normal observation path.
        void this.ready.catch(() => { });
        this.#stop = clock.later(5000, () => {
            expired();
            this.fail(new CanvasPreparationTimeout());
        });
    }
    /** Complete only after the native host acknowledges preparation of the requested content. */
    complete() {
        const complete = this.#complete;
        this.#clear();
        complete?.();
    }
    /** Preserve the first preparation failure; later callbacks cannot change the outcome. */
    fail(error) {
        const fail = this.#fail;
        this.#clear();
        fail?.(error);
    }
    /** Cancel a staged attachment even when the native host never answered. */
    dispose() {
        this.fail(new Error('Native canvas attachment was released before preparation completed.'));
    }
    /** Release the single deadline and both outcome callbacks at the terminal transition. */
    #clear() {
        this.#stop?.();
        this.#stop = null;
        this.#complete = null;
        this.#fail = null;
    }
}
