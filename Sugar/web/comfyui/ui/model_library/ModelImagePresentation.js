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
/**
 * How every model image is shown, on every surface that shows one: field banners, wall tiles,
 * version rows, download thumbnails and recommendation portraits.
 *
 * - The image on screen changes only when its replacement has been decoded, so nothing blanks in
 *   between. A newer replacement supersedes one still decoding.
 * - A replacement that fails leaves the image on screen.
 * - Decoded images are kept by address. An address already decoded is shown at once, wherever it
 *   is asked for again: a tile scrolled back into view, the same model on another surface.
 * - A failed address is tried again only when the record it came from changes: the catalog
 *   publishing the model's image again, or a different address.
 */
/** How many decoded images are kept after the surfaces showing them have gone. */
export const DECODED_MODEL_IMAGES = 512;
const decoded = new Map();
const decoding = new Map();
/** Whether an address has been decoded and is still kept. */
export function modelImageDecoded(url) {
    return decoded.has(url);
}
/** Forget every kept image; for tests and for a page that is being torn down. */
export function forgetDecodedModelImages() {
    decoded.clear();
    decoding.clear();
}
/**
 * Decode an image in the browser and keep it. Addresses are content hashes served as immutable,
 * so a kept image never goes stale; surfaces asking for the same address share one decode.
 */
export const decodeModelImage = (url) => {
    const kept = decoded.get(url);
    if (kept) {
        // Most recently used last, so the oldest is the first to go.
        decoded.delete(url);
        decoded.set(url, kept);
        return Promise.resolve(true);
    }
    const running = decoding.get(url);
    if (running)
        return running;
    const image = new Image();
    image.decoding = 'async';
    image.referrerPolicy = 'no-referrer';
    image.src = url;
    const settled = image.decode().then(() => {
        decoded.set(url, image);
        for (const oldest of decoded.keys()) {
            if (decoded.size <= DECODED_MODEL_IMAGES)
                break;
            decoded.delete(oldest);
        }
        return true;
    }, () => false);
    decoding.set(url, settled);
    void settled.then(() => decoding.delete(url));
    return settled;
};
const NEVER = Symbol('never failed');
/** One surface's image. `changed` is called whenever what it should draw changes. */
export class ModelImagePresentation {
    changed;
    decode;
    ready;
    #shown = null;
    #wanted = null;
    #failed = NEVER;
    #pending = false;
    #attempt = 0;
    #disposed = false;
    constructor(changed, decode = decodeModelImage, ready = modelImageDecoded) {
        this.changed = changed;
        this.decode = decode;
        this.ready = ready;
    }
    /** The address to draw, or null while there is none to draw. */
    get shown() {
        return this.#shown;
    }
    /** Whether an image is being decoded with nothing on screen to keep meanwhile. */
    get loading() {
        return this.#pending && this.#shown === null;
    }
    /** Whether there is nothing to draw because the wanted image cannot be shown. */
    get unavailable() {
        return this.#shown === null && !this.#pending;
    }
    /**
     * Ask for `url`, which came from `record`. Asking again for an address that failed does nothing
     * until it comes from a different record.
     */
    show(url, record = url) {
        if (this.#disposed)
            return;
        if (url === this.#wanted && (this.#failed === NEVER || this.#failed === record))
            return;
        this.#wanted = url;
        this.#failed = NEVER;
        const attempt = ++this.#attempt;
        if (url === null) {
            this.#pending = false;
            this.#shown = null;
            this.changed();
            return;
        }
        if (this.ready(url)) {
            this.#pending = false;
            this.#shown = url;
            this.changed();
            return;
        }
        this.#pending = true;
        this.changed();
        void this.decode(url).then((shown) => {
            if (this.#disposed || attempt !== this.#attempt)
                return;
            this.#pending = false;
            if (shown)
                this.#shown = url;
            else
                this.#failed = record;
            this.changed();
        });
    }
    /** Stop reporting; a decode still running keeps its image for the next surface that asks. */
    dispose() {
        this.#disposed = true;
    }
}
