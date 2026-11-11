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
/** Bind portable canvas operations to the authenticated panel navigation without claiming its transport. */
import { CanvasClient, CanvasMaskClient, } from './CanvasRuntime.js';
/** Keep the optional native canvas unavailable until the panel has authenticated and negotiated it. */
export class EmbeddedCanvasConnection {
    /** Negotiate output presentation through this same canvas session. */
    get surface() {
        return this.#client?.surface ?? null;
    }
    /** Borrow the existing authenticated request sequence for optional geometry operations. */
    get geometry() {
        return this.#client?.geometry ?? null;
    }
    #client = null;
    #masks = null;
    /** Expose source reads on the authenticated canvas connection without a second request sequence. */
    get sources() {
        return this.#client?.sources ?? null;
    }
    /** Expose editing commands on the authenticated session; adapters gate them by capability. */
    get editing() {
        return this.#client?.editing ?? null;
    }
    /** Expose workflow surface provisioning on the same correlated request sequence. */
    get workflow() {
        return this.#client?.workflow ?? null;
    }
    /** Expose mask transport only after the panel navigation has authenticated. */
    get masks() {
        return this.#masks;
    }
    #stop = null;
    #window = null;
    #report = null;
    #read = {};
    #refreshing = null;
    #refreshAgain = false;
    #surfaceDiscovered = false;
    #disposed = false;
    /** Report the current discovery's completion, including unavailable or failed reads. */
    get surfaceDiscovered() {
        return this.#surfaceDiscovered;
    }
    /** Read context and optional surface capabilities before publishing panel readiness. */
    connect(transport, window, report) {
        if (this.#disposed)
            throw new Error('The native canvas connection is closed.');
        if (this.#client)
            return;
        const client = new CanvasClient(transport.session, {
            send: (message) => transport.send(message),
            later: (ms, callback) => {
                const timer = window.setTimeout(callback, ms);
                return () => window.clearTimeout(timer);
            },
            report,
            // The native host owns this page and answers every request, so its loss ends the page; a
            // slow host is never a dead one.
            settlesRequests: true,
        });
        this.#client = client;
        this.#window = window;
        this.#report = report;
        this.#masks = new CanvasMaskClient(transport.session, {
            send: (message) => transport.send(message),
            report,
            later: (ms, callback) => {
                const timer = window.setTimeout(callback, ms);
                return () => window.clearTimeout(timer);
            },
        }, () => client.readCanvas()?.id ?? '');
        this.#stop = transport.subscribe((value) => {
            client.receive(value);
            this.#masks?.receive(value);
        });
        this.refresh();
    }
    /** Coalesce native discovery after an actual backend connection transition. */
    refresh() {
        const client = this.#client;
        if (!client)
            return;
        if (this.#refreshing) {
            this.#refreshAgain = true;
            return;
        }
        const pending = this.#readContext(true).then(() => { }, (error) => {
            if (this.#client === client)
                this.#report?.(error);
        });
        this.#refreshing = pending;
        void pending.then(() => {
            if (this.#refreshing !== pending)
                return;
            this.#refreshing = null;
            if (this.#refreshAgain) {
                this.#refreshAgain = false;
                this.refresh();
            }
        });
    }
    /** Advertise only the capability descriptor received from the current native session. */
    readCanvas() {
        return this.#client?.readCanvas() ?? null;
    }
    /** Revoke native grants and in-flight refreshes when the backend connection actually leaves. */
    disconnected() {
        this.#read = {};
        this.#surfaceDiscovered = false;
        this.#refreshAgain = false;
        this.#refreshing = null;
        this.#client?.withdrawSurface();
        this.#window?.document.dispatchEvent(new Event('sugarcubes-canvas-discovered'));
    }
    /** Capture a native document identity before preparing an image transfer. */
    readContext() {
        return this.#readContext(false);
    }
    /** Fence current discovery progress separately from the active native surface grant. */
    async #readContext(refresh) {
        const client = this.#client;
        if (!client)
            throw new Error('The native canvas is unavailable.');
        const read = {};
        this.#read = read;
        this.#surfaceDiscovered = false;
        let context;
        try {
            context = await (refresh ? client.refreshContext() : client.readContext());
        }
        finally {
            // Discovery concludes whether the host offered, refused or failed to describe a surface.
            if (this.#client === client && this.#read === read) {
                this.#surfaceDiscovered = true;
                this.#window?.document.dispatchEvent(new Event('sugarcubes-canvas-discovered'));
            }
        }
        if (this.#client === client && this.#read === read)
            this.#window?.document.dispatchEvent(new Event('sugarcubes-canvas-ready'));
        return context;
    }
    /** Delegate a mutation once to the correlated session client. */
    async insertImage(image, cancellation) {
        if (!this.#client)
            return { status: 'unavailable', message: 'The native canvas is unavailable.' };
        return await this.#client.insertImage(image, cancellation);
    }
    /** Retire this page's connection permanently before releasing pending requests. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#stop?.();
        this.#stop = null;
        this.#client?.dispose();
        this.#client = null;
        this.#window = null;
        this.#report = null;
        this.#refreshing = null;
        this.#refreshAgain = false;
        this.#masks?.dispose();
        this.#masks = null;
    }
}
