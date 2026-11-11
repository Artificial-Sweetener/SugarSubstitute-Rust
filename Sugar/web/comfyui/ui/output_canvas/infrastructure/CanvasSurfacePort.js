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
/** Qualify overlay commands and revisions against one current canvas attachment. */
import { decodeCanvasCommandRequest } from './CanvasCommandDecoder.js';
import { decodeCanvasSurfaceGeometry } from './CanvasGeometryDecoder.js';
import { authorizeCanvasMessage } from './CanvasMessageAuthority.js';
/** Keep surface revisions independent of lightweight selection-state revisions. */
export class CanvasSurfacePort {
    options;
    capabilities;
    #listeners = new Set();
    #address;
    #snapshot;
    #request = 0;
    #issued = 0;
    #active = null;
    #disposed = false;
    /** Seal the first attachment address and explicit feature capabilities. */
    constructor(initial, options) {
        this.options = options;
        if (!Number.isSafeInteger(initial.revision) || initial.revision < 0)
            throw new Error('Canvas surface revision is invalid.');
        this.#address = initial.address;
        this.#snapshot = Object.freeze(initial);
        this.capabilities = Object.freeze({
            schema: 1,
            commands: Object.freeze([...options.capabilities.commands]),
        });
    }
    /** Read the latest published surface snapshot without consulting ambient host state. */
    read() {
        return this.#snapshot;
    }
    /** Subscribe only to this attachment, with consumer-owned release. */
    subscribe(listener) {
        if (this.#disposed)
            return () => { };
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Reserve one request number across every control and programmatic entry point. */
    createRequest(command, expectedRevision) {
        if (this.#disposed || !this.options.accepts())
            throw new Error('Canvas attachment is unavailable.');
        if (this.#issued === Number.MAX_SAFE_INTEGER)
            throw new Error('Canvas request identity exhausted.');
        return Object.freeze({
            schema: 1,
            address: this.#address,
            requestId: ++this.#issued,
            expectedRevision,
            command,
        });
    }
    /** Publish an authoritative presentation change with a monotonically increasing local revision. */
    publish(snapshot) {
        if (this.#disposed || !this.options.accepts())
            return;
        if (this.#snapshot.revision === Number.MAX_SAFE_INTEGER)
            throw new Error('Canvas surface revision exhausted.');
        this.#snapshot = Object.freeze({
            ...snapshot,
            address: this.#address,
            revision: this.#snapshot.revision + 1,
        });
        for (const listener of this.#listeners)
            listener(this.#snapshot);
    }
    /** Decode every local intent as an external boundary and reject duplicate/stale deliveries. */
    async dispatch(value) {
        const revision = this.#snapshot.revision;
        if (this.#disposed || !this.options.accepts())
            return { status: 'cancelled', revision };
        const request = decodeCanvasCommandRequest(value, this.options.origin, {
            origin: this.options.origin,
            address: this.#address,
            capabilities: this.capabilities,
        });
        if (!request)
            return { status: 'unsupported', revision };
        if (request.requestId <= this.#request || request.expectedRevision !== revision)
            return { status: 'stale', revision };
        const previous = this.#active;
        const controller = new AbortController();
        this.#request = request.requestId;
        this.#issued = Math.max(this.#issued, request.requestId);
        this.#active = controller;
        const scope = Object.freeze({
            request,
            signal: controller.signal,
            current: () => !this.#disposed &&
                !controller.signal.aborted &&
                this.#active === controller &&
                this.options.accepts(),
        });
        // Install the new identity before notifying older work: abort callbacks can reenter dispatch.
        previous?.abort();
        try {
            if (!scope.current())
                return this.#invalidOutcome(request.requestId);
            const applied = await this.options.apply(request.command, scope);
            if (!scope.current())
                return this.#invalidOutcome(request.requestId);
            return { status: applied ? 'applied' : 'unsupported', revision: this.#snapshot.revision };
        }
        catch (error) {
            if (!scope.current()) {
                if (!(controller.signal.aborted &&
                    error instanceof DOMException &&
                    error.name === 'AbortError'))
                    this.options.report(error, request);
                return this.#invalidOutcome(request.requestId);
            }
            this.options.report(error, request);
            return {
                status: 'error',
                revision: this.#snapshot.revision,
                message: 'The canvas action could not be completed.',
            };
        }
        finally {
            if (this.#active === controller)
                this.#active = null;
            controller.abort();
        }
    }
    /** Distinguish replacement by a newer command from retirement of this attachment. */
    #invalidOutcome(requestId) {
        return {
            status: this.#disposed || !this.options.accepts()
                ? 'cancelled'
                : requestId !== this.#request
                    ? 'stale'
                    : 'cancelled',
            revision: this.#snapshot.revision,
        };
    }
    /** Validate density, visibility, and coordinates before updating this surface. */
    updateSurface(address, geometry) {
        if (this.#disposed || !this.options.accepts())
            return;
        const decoded = decodeCanvasSurfaceGeometry({ schema: 1, address, geometry }, this.options.origin, { origin: this.options.origin, address: this.#address });
        if (decoded)
            this.options.geometry(decoded);
    }
    /** Release only a matching current attachment, leaving replacement surfaces untouched. */
    releaseSurface(address) {
        if (this.#disposed || !this.options.accepts())
            return;
        if (authorizeCanvasMessage({ schema: 1, address }, this.options.origin, {
            origin: this.options.origin,
            address: this.#address,
        }))
            this.options.release();
    }
    /** End delivery without implicitly retiring a view or another attachment. */
    dispose() {
        this.#disposed = true;
        const active = this.#active;
        this.#active = null;
        active?.abort();
        this.#listeners.clear();
    }
}
