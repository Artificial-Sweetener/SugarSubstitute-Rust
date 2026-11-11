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
/** Retain decoded Cube icons for the page lifetime, independently of workflow runtimes. */
import { BudgetedMemoryCache } from './memory/BudgetedMemoryCache.js';
/**
 * Share decoded icons between every runtime, painter and workflow.
 *
 * Icons are keyed by URL plus content identity, so a revised Cube resolves a new image while
 * unchanged Cubes keep theirs across workflow switches. Waiters are held only until their image
 * settles, so a replaced runtime is never retained by a cached icon.
 */
export class CubeIconImageStore {
    #icons;
    /** Join the page memory allowance as decoded-image storage. */
    constructor(registry) {
        this.#icons = new BudgetedMemoryCache({
            id: 'cube-icons',
            registry,
            release: (icon) => {
                icon.waiting.clear();
                icon.entry.image?.removeAttribute('src');
            },
        });
    }
    /** Return the shared entry for one icon, starting its load once and notifying on settle. */
    acquire(request, createImage, settled) {
        const key = `${request.url}\u0000${request.identity}`;
        const stored = this.#icons.get(key);
        if (stored) {
            if (stored.entry.status === 'loading' && settled)
                stored.waiting.add(settled);
            return stored.entry;
        }
        if (!createImage) {
            const unavailable = { status: 'error', image: null };
            this.#icons.set(key, { entry: unavailable, waiting: new Set() }, { tier: 2, bytes: 0 });
            return unavailable;
        }
        const image = createImage();
        const icon = { entry: { status: 'loading', image }, waiting: new Set() };
        if (settled)
            icon.waiting.add(settled);
        const settle = (status) => {
            icon.entry.status = status;
            if (status === 'ready')
                this.#icons.resize(key, Math.max(0, image.naturalWidth * image.naturalHeight * 4));
            const waiting = [...icon.waiting];
            icon.waiting.clear();
            for (const listener of waiting)
                listener(request.url, icon.entry);
        };
        image.onload = () => settle('ready');
        image.onerror = () => settle('error');
        this.#icons.set(key, icon, { tier: 2, bytes: 0 });
        image.src = request.url;
        return icon.entry;
    }
    /** Release every retained icon with the owning page. */
    dispose() {
        this.#icons.dispose();
    }
}
