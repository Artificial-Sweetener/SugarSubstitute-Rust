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
/** Decode ordinary browser images while owning only this surface's elements and preview URLs. */
import { canvasImageBytes, } from '../domain/CanvasImageResource.js';
/** Keep platform decode and URL release behind the generic image-loading port. */
export class BrowserCanvasImageLoader {
    document;
    urls;
    #releases = new Set();
    #urls = new Set();
    #disposed = false;
    /** Inject the attachment document and URL allocator for explicit window ownership. */
    constructor(document, urls) {
        this.document = document;
        this.urls = urls;
    }
    /** Start one admitted ordinary image without copying pixels or retaining preview bytes. */
    start(input) {
        if (this.#disposed)
            throw new Error('Canvas image loader is disposed.');
        const image = this.document.createElement('img');
        image.decoding = 'async';
        image.draggable = false;
        image.alt = '';
        let url = null;
        let settled = false;
        let released = false;
        let ready = () => { };
        let failed = () => { };
        const completion = new Promise((resolve, reject) => {
            ready = resolve;
            failed = reject;
        });
        /** Relinquish this allocation once, including a platform decode that settles after cancellation. */
        const release = () => {
            if (released)
                return;
            released = true;
            image.removeEventListener('load', loaded);
            image.removeEventListener('error', errored);
            image.removeAttribute('src');
            image.remove();
            this.#releases.delete(cancel);
            if (url !== null) {
                this.#urls.delete(url);
                this.urls.revoke(url);
                url = null;
            }
        };
        /** Reject a failed/canceled load while retaining its original diagnostic exception. */
        const reject = (error) => {
            if (settled)
                return;
            settled = true;
            release();
            failed(error);
        };
        /** Accept valid intrinsic geometry only while this exact allocation remains owned. */
        const accept = () => {
            if (settled || released)
                return;
            try {
                const dimensions = Object.freeze({
                    width: image.naturalWidth,
                    height: image.naturalHeight,
                });
                canvasImageBytes(dimensions);
                settled = true;
                ready(Object.freeze({ handle: image, dimensions, release }));
            }
            catch (error) {
                reject(error);
            }
        };
        /** Support hosts without decode while preferring decode completion when available. */
        const loaded = () => {
            if (typeof image.decode !== 'function')
                accept();
        };
        /** Translate browser load failure to a recoverable platform error. */
        const errored = () => {
            reject(new Error('Browser image loading failed.'));
        };
        /** End both promise delivery and platform ownership at this surface's lifetime boundary. */
        const cancel = () => {
            reject(new Error('Canvas image loading canceled.'));
            release();
        };
        this.#releases.add(cancel);
        image.addEventListener('load', loaded);
        image.addEventListener('error', errored);
        try {
            if (input.payload.kind === 'preview') {
                url = this.urls.create(input.payload.blob);
                this.#urls.add(url);
                image.src = url;
            }
            else {
                const target = new URL(input.payload.url, this.document.baseURI);
                const origin = new URL(this.document.baseURI).origin;
                if (!['http:', 'https:'].includes(target.protocol) || target.origin !== origin)
                    throw new Error('Canvas file URL must use the current host origin.');
                image.src = target.href;
            }
            if (typeof image.decode === 'function')
                void image.decode().then(accept, reject);
        }
        catch (error) {
            reject(error);
            // A start failure has no returned promise consumer; contain its rejection before rethrowing.
            void completion.catch(() => { });
            throw error;
        }
        return {
            completion,
            cancel,
        };
    }
    /** Count only image elements and URLs explicitly owned by this surface. */
    diagnostics() {
        return Object.freeze({ elements: this.#releases.size, previewUrls: this.#urls.size });
    }
    /** Release every platform allocation at the attachment/window lifetime boundary. */
    dispose() {
        this.#disposed = true;
        for (const release of [...this.#releases])
            release();
    }
}
