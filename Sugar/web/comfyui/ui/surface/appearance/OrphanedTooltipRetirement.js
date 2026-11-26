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
 * Remove a ComfyUI tooltip that nothing is left to hide.
 *
 * ComfyUI's tooltips come from PrimeVue's directive, which shows one a moment after the pointer
 * enters its element and removes it when the pointer leaves that element. Two things leave a
 * tooltip on the page for good. An element rebuilt during that moment still gets its tooltip,
 * and no element remains whose leave would remove it. And the directive keeps one timer for
 * every element, so a pointer that crosses two of them quickly shows the first one's tooltip
 * after the pointer has left it.
 *
 * A tooltip stays only while the element that showed it is on the page and the pointer or the
 * keyboard focus is in it, or the pointer is on the tooltip itself.
 */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
/** The properties PrimeVue's tooltip directive keeps on the element a tooltip belongs to. */
const OWNER_SELECTOR = '[data-pd-tooltip]';
const SHOWN_TOOLTIP_ID = '$_ptooltipId';
const HIDE_DELAY = '$_ptooltipHideDelay';
/** Past the directive's own removal, which runs after its element's hide delay. */
const SETTLE_MS = 150;
/** Watch the tooltips PrimeVue adds to the body and remove each one that has lost its element. */
export class OrphanedTooltipRetirement {
    document;
    #observer;
    #shown = new Set();
    #pending = new Map();
    #pointer = null;
    constructor(document) {
        this.document = document;
        document.addEventListener('pointermove', this.#onMove, { capture: true, passive: true });
        // PrimeVue's tooltips carry the identity their element names; SugarCubes' own do not.
        this.#observer = new HostSurfaceObserver(document.body, '.p-tooltip[id]', (changes) => {
            for (const change of changes) {
                if (change.kind === 'mounted')
                    this.#mounted(change.element);
                else if (change.kind === 'removed')
                    this.#forget(change.element);
            }
        });
    }
    dispose() {
        this.#observer.disconnect();
        this.document.removeEventListener('pointermove', this.#onMove, { capture: true });
        for (const timer of this.#pending.values())
            clearTimeout(timer);
        this.#pending.clear();
        this.#shown.clear();
    }
    /** A tooltip whose element is already gone has nothing to wait for. */
    #mounted(tooltip) {
        if (!this.#owner(tooltip)) {
            tooltip.remove();
            return;
        }
        this.#shown.add(tooltip);
        this.#review(tooltip);
    }
    #forget(tooltip) {
        this.#shown.delete(tooltip);
        const timer = this.#pending.get(tooltip);
        if (timer !== undefined)
            clearTimeout(timer);
        this.#pending.delete(tooltip);
    }
    /** The element on the page that names this tooltip as the one it is showing. */
    #owner(tooltip) {
        for (const element of this.document.querySelectorAll(OWNER_SELECTOR))
            if (Reflect.get(element, SHOWN_TOOLTIP_ID) === tooltip.id)
                return element;
        return null;
    }
    /** Whether the pointer or the keyboard focus still gives the tooltip a reason to be shown. */
    #held(tooltip, owner) {
        const pointer = this.#pointer;
        // A tooltip opened before any pointer movement was opened from the keyboard.
        if (!pointer)
            return true;
        return (owner.contains(pointer) ||
            tooltip.contains(pointer) ||
            owner.contains(this.document.activeElement));
    }
    /**
     * Remove a tooltip that has lost its element at once. One the pointer has left is given the
     * time PrimeVue takes to remove it itself, and is removed only if it is still there.
     */
    #review(tooltip) {
        if (!tooltip.isConnected) {
            this.#forget(tooltip);
            return;
        }
        const owner = this.#owner(tooltip);
        if (!owner) {
            this.#forget(tooltip);
            tooltip.remove();
            return;
        }
        if (this.#held(tooltip, owner) || this.#pending.has(tooltip))
            return;
        const delay = Reflect.get(owner, HIDE_DELAY);
        this.#pending.set(tooltip, setTimeout(() => {
            this.#pending.delete(tooltip);
            const current = tooltip.isConnected ? this.#owner(tooltip) : null;
            if (tooltip.isConnected && (!current || !this.#held(tooltip, current))) {
                this.#forget(tooltip);
                tooltip.remove();
            }
        }, (typeof delay === 'number' && delay > 0 ? delay : 0) + SETTLE_MS));
    }
    #onMove = (event) => {
        this.#pointer = event.target instanceof Node ? event.target : null;
        for (const tooltip of [...this.#shown])
            this.#review(tooltip);
    };
}
