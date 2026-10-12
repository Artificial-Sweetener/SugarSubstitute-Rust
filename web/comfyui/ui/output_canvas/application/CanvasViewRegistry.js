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
/** Own view state and transactional attachments independently of surface resources. */
import { canvasViewId, sameCanvasAttachment, surfaceAttachmentId, } from '../domain/CanvasIdentity.js';
import { freezeCanvasViewState, initialCanvasViewState, } from '../domain/CanvasViewState.js';
/** Keep result sessions, selection views, and replaceable attachments distinct. */
export class CanvasViewRegistry {
    nextKey;
    #views = new Map();
    /** Inject identity generation without giving domain code access to a browser. */
    constructor(nextKey) {
        this.nextKey = nextKey;
    }
    /** Create an additional view with independent selection and inspection intent. */
    create(sessionId) {
        const viewId = canvasViewId(this.nextKey());
        if (this.#views.has(viewId))
            throw new Error('Canvas view identity was reused.');
        const state = initialCanvasViewState({ sessionId, viewId });
        this.#views.set(viewId, {
            state,
            attachment: null,
            pending: null,
            generation: 0,
            listeners: new Set(),
        });
        return state;
    }
    /** Read immutable authority-owned view state without allocating presentation. */
    read(viewId) {
        return this.#require(viewId).state;
    }
    /** Publish one coherent transition while preserving view and session identity. */
    update(viewId, change) {
        const record = this.#require(viewId);
        const next = change(record.state);
        if (next === record.state)
            return next;
        if (next.viewId !== viewId || next.sessionId !== record.state.sessionId) {
            throw new TypeError('A canvas state transition cannot change its identity.');
        }
        record.state = freezeCanvasViewState(next, record.state.revision + 1);
        for (const listener of record.listeners)
            listener(record.state);
        return record.state;
    }
    /** Subscribe to this view only, with explicit consumer-owned cleanup. */
    subscribe(viewId, listener) {
        const record = this.#require(viewId);
        record.listeners.add(listener);
        return () => record.listeners.delete(listener);
    }
    /** Allocate a first attachment or a pending replacement without retiring the old surface. */
    prepareAttachment(viewId, windowId) {
        const record = this.#require(viewId);
        if (record.pending)
            throw new Error('A canvas attachment transaction is already pending.');
        const address = Object.freeze({
            sessionId: record.state.sessionId,
            viewId,
            windowId,
            attachmentId: surfaceAttachmentId(this.nextKey()),
            generation: ++record.generation,
        });
        record.pending = address;
        return address;
    }
    /** Commit a ready destination and return the previous surface for explicit retirement. */
    commitAttachment(address) {
        const record = this.#require(address.viewId);
        if (!record.pending || !sameCanvasAttachment(record.pending, address)) {
            throw new Error('Canvas attachment acknowledgment is stale.');
        }
        const previous = record.attachment;
        record.attachment = record.pending;
        record.pending = null;
        return previous;
    }
    /** Abort a failed destination while retaining the current surface and view state. */
    cancelAttachment(address) {
        const record = this.#views.get(address.viewId);
        if (!record?.pending || !sameCanvasAttachment(record.pending, address))
            return false;
        record.pending = null;
        return true;
    }
    /** Test the full delivery address instead of accepting a view ID from a retired window. */
    accepts(address) {
        const attachment = this.#views.get(address.viewId)?.attachment;
        return Boolean(attachment && sameCanvasAttachment(attachment, address));
    }
    /** Release only the named current surface; stale teardown leaves its replacement intact. */
    releaseAttachment(address) {
        const record = this.#views.get(address.viewId);
        if (!record?.attachment || !sameCanvasAttachment(record.attachment, address))
            return false;
        record.attachment = null;
        return true;
    }
    /** Retire an explicitly closed view without changing any other view or result session. */
    remove(viewId) {
        const record = this.#views.get(viewId);
        if (!record)
            return;
        record.listeners.clear();
        this.#views.delete(viewId);
    }
    /** Reject operations on a retired view rather than recreating it through delayed work. */
    #require(viewId) {
        const record = this.#views.get(viewId);
        if (!record)
            throw new Error('Canvas view is unavailable.');
        return record;
    }
}
