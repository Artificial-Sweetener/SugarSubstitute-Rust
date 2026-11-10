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
/** Settle one native attachment's preparation from observed host events only, never from elapsed time. */
import { CanvasPreparationReleased, } from '../domain/CanvasPreparation.js';
/** Observe real host readiness once, retaining the first observed outcome for the attachment owner. */
export class NativeCanvasReadiness {
    ready;
    #outcome = { state: 'pending' };
    #resolve = null;
    #reject = null;
    /** Open one pending preparation; only the host's reply, its failure or a release ends it. */
    constructor() {
        this.ready = new Promise((resolve, reject) => {
            this.#resolve = resolve;
            this.#reject = reject;
        });
        // Constructor failure may prevent the attachment owner from receiving this promise.
        // Keep the original promise rejected for that owner's normal observation path.
        void this.ready.catch(() => { });
    }
    /** The preparation's current outcome; `pending` until the host answers or the attachment ends. */
    get outcome() {
        return this.#outcome;
    }
    /** Complete only after the native host acknowledges preparation of the requested content. */
    complete() {
        if (this.#outcome.state !== 'pending')
            return;
        this.#outcome = { state: 'ready' };
        this.#settle().resolve?.();
    }
    /** Preserve the first observed failure; later callbacks cannot change the outcome. */
    fail(failure) {
        if (this.#outcome.state !== 'pending')
            return;
        this.#outcome = { state: 'failed', failure };
        this.#settle().reject?.(failure);
    }
    /** Cancel a staged attachment even when the native host never answered. */
    dispose() {
        if (this.#outcome.state !== 'pending')
            return;
        this.#outcome = { state: 'released' };
        this.#settle().reject?.(new CanvasPreparationReleased());
    }
    /** Hand out the one-shot callbacks and drop them, so a settled preparation retains nothing. */
    #settle() {
        const callbacks = { resolve: this.#resolve, reject: this.#reject };
        this.#resolve = null;
        this.#reject = null;
        return callbacks;
    }
}
