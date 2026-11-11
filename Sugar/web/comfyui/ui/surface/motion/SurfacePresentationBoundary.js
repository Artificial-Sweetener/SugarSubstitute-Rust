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
/** Yield native presentation frames without polling geometry or retaining retired surfaces. */
export class SurfacePresentationBoundary {
    #window;
    #frame = null;
    #resolve = null;
    /** Bind frame scheduling to the surface's own browser document. */
    constructor(documentRef) {
        this.#window = documentRef.defaultView;
    }
    /** Allow queued host styling and its coalesced resize delivery to precede endpoint measurement. */
    settled(frames = 2) {
        this.cancel();
        const windowRef = this.#window;
        if (!windowRef?.requestAnimationFrame)
            return Promise.resolve();
        return new Promise((resolve) => {
            this.#resolve = resolve;
            const finish = () => {
                if (this.#resolve !== resolve)
                    return;
                this.#frame = null;
                this.#resolve = null;
                resolve();
            };
            this.#frame = windowRef.requestAnimationFrame(() => {
                if (this.#resolve !== resolve)
                    return;
                if (frames === 1)
                    finish();
                else
                    this.#frame = windowRef.requestAnimationFrame(finish);
            });
        });
    }
    /** Release pending preparation on reversal or teardown, including its waiting caller. */
    cancel() {
        if (this.#frame !== null)
            this.#window?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        this.#resolve?.();
        this.#resolve = null;
    }
}
