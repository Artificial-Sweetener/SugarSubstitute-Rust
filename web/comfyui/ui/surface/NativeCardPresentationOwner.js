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
/** Transfer identified native cards between presentations without duplicating widget mounts. */
import { settleCubePromptTextareaGeometry } from './CubePromptTextareaAutosizer.js';
const PRESENTATION_OWNER = Symbol('SugarCubes native card presentation owner');
const GEOMETRY_PROPERTIES = ['position', 'left', 'top', 'width', 'height', 'transform'];
/** Own exclusive presentation loans and invalidate them before native source teardown. */
export class NativeCardPresentationOwner {
    #onReturned;
    #cells = new Set();
    #transfers = new Map();
    #sourceHeights = new Map();
    #resetting = false;
    /** Resume source geometry only after its last card has returned. */
    constructor(onReturned) {
        this.#onReturned = onReturned;
    }
    /** Suspend source layout while any exact native cell belongs to another presentation. */
    get borrowed() {
        return this.#transfers.size > 0;
    }
    /** Identify an exact cell whose native renderer remains owned by this source. */
    transferFor(cell) {
        return this.#transfers.get(cell) ?? null;
    }
    /** Publish capabilities only on this source's current identified cells. */
    reset(cells) {
        this.#resetting = true;
        const transfers = [...this.#transfers.values()];
        NativeCardPresentationTransfer.restoreAll(transfers);
        for (const cell of this.#cells)
            Reflect.deleteProperty(cell, PRESENTATION_OWNER);
        this.#cells.clear();
        this.#sourceHeights.clear();
        for (const cell of cells) {
            this.#cells.add(cell);
            Reflect.set(cell, PRESENTATION_OWNER, this);
        }
        this.#resetting = false;
        for (const transfer of transfers)
            transfer.invalidate();
    }
    /** Read source geometry before a borrower creates headers or changes native widths. */
    measureSources() {
        if (this.borrowed)
            return;
        for (const source of this.#cells) {
            const computed = source.ownerDocument.defaultView?.getComputedStyle(source);
            this.#sourceHeights.set(source, computed?.height || `${String(source.offsetHeight)}px`);
        }
    }
    /** Reserve a card after reading source geometry, before any presentation writes. */
    prepare(cell, onInvalidated) {
        if (!this.#cells.has(cell) || this.#transfers.has(cell) || !cell.parentElement)
            return null;
        if (!this.#sourceHeights.has(cell))
            this.measureSources();
        const transfer = new NativeCardPresentationTransfer(cell, this.#sourceHeights.get(cell) ?? '0px', onInvalidated, () => {
            this.#transfers.delete(cell);
            if (!this.borrowed) {
                this.#sourceHeights.clear();
                if (!this.#resetting)
                    this.#onReturned();
            }
        });
        this.#transfers.set(cell, transfer);
        return transfer;
    }
    /** Return all cells before their renderer releases native controls. */
    dispose() {
        this.reset([]);
    }
}
/** Retain a source reservation and move the original cell with all native listeners intact. */
export class NativeCardPresentationTransfer {
    element;
    #origin;
    #geometry;
    #onReturned;
    #onInvalidated;
    #active = true;
    #moved = false;
    #invalidated = false;
    /** Capture unscaled source dimensions while all cards remain in their original presentation. */
    constructor(element, height, onInvalidated, onReturned) {
        this.element = element;
        this.#onReturned = onReturned;
        this.#onInvalidated = onInvalidated;
        this.#geometry = GEOMETRY_PROPERTIES.map((name) => ({
            name,
            value: element.style.getPropertyValue(name),
            priority: element.style.getPropertyPriority(name),
        }));
        this.#origin = element.ownerDocument.createElement('div');
        this.#origin.dataset.cubeNodeId = element.dataset.cubeNodeId ?? '';
        this.#origin.dataset.scCardOrigin = '';
        this.#origin.setAttribute('aria-hidden', 'true');
        for (const property of this.#geometry)
            if (property.value)
                this.#origin.style.setProperty(property.name, property.value, property.priority);
        this.#origin.style.height = height;
        this.#origin.style.visibility = 'hidden';
        this.#origin.style.pointerEvents = 'none';
    }
    /** Expose invalidation without allowing a second source or presentation owner. */
    get active() {
        return this.#active;
    }
    /** Move the complete cell so icons, forwarded widgets, and header controls retain their owners. */
    move(target) {
        if (!this.#active || this.#moved)
            return;
        this.#moved = true;
        this.element.before(this.#origin);
        relocateCard(this.element, target);
        Object.assign(this.element.style, {
            position: 'relative',
            left: '0px',
            top: '0px',
            width: '100%',
            height: 'auto',
            transform: 'none',
        });
    }
    /** Return the same cell in source order and restore only geometry owned by the transfer. */
    restore() {
        NativeCardPresentationTransfer.restoreAll([this]);
    }
    /** Restore every native width before prompt reads, then resume each source's layout once. */
    static restoreAll(transfers) {
        const active = [...new Set(transfers)].filter((transfer) => transfer.#active);
        const returned = [];
        for (const transfer of active) {
            transfer.#active = false;
            const { element } = transfer;
            if (transfer.#moved && transfer.#origin.parentElement) {
                relocateCard(element, transfer.#origin.parentElement, transfer.#origin);
                transfer.#origin.remove();
                for (const { name, value, priority } of transfer.#geometry) {
                    if (value)
                        element.style.setProperty(name, value, priority);
                    else
                        element.style.removeProperty(name);
                }
                returned.push(element);
            }
        }
        settleCubePromptTextareaGeometry(returned);
        for (const transfer of active)
            transfer.#onReturned();
    }
    /** Notify a borrower once when its original renderer is about to replace the source. */
    invalidate() {
        if (this.#invalidated)
            return;
        this.#invalidated = true;
        this.#onInvalidated();
    }
}
/** Identify an unchanged Cube-card presentation retained by its source renderer. */
export function isBorrowedNativeCardPresentation(root) {
    const cell = root.closest('.sugarcubes-cube-face__node-card');
    if (!cell)
        return false;
    const owner = Reflect.get(cell, PRESENTATION_OWNER);
    return owner instanceof NativeCardPresentationOwner && owner.transferFor(cell) !== null;
}
/** Return an identified presentation's loans as one geometry batch before releasing its mounts. */
export function restoreNativeCardPresentations(cells) {
    const transfers = [];
    for (const cell of cells) {
        const owner = Reflect.get(cell, PRESENTATION_OWNER);
        const transfer = owner instanceof NativeCardPresentationOwner ? owner.transferFor(cell) : null;
        if (transfer)
            transfers.push(transfer);
    }
    NativeCardPresentationTransfer.restoreAll(transfers);
}
/** Batch identified source reads before constructing either presentation's new content. */
export function measureNativeCardPresentationSources(cells) {
    const owners = new Set();
    for (const cell of cells) {
        const owner = Reflect.get(cell, PRESENTATION_OWNER);
        if (owner instanceof NativeCardPresentationOwner)
            owners.add(owner);
    }
    for (const owner of owners)
        owner.measureSources();
}
/** Read the capability of an already identified source cell without a global surface registry. */
export function prepareNativeCardPresentation(cell, onInvalidated) {
    const owner = Reflect.get(cell, PRESENTATION_OWNER);
    return owner instanceof NativeCardPresentationOwner ? owner.prepare(cell, onInvalidated) : null;
}
/** Preserve a focused control while reparenting its existing native DOM. */
function relocateCard(element, target, before = null) {
    const focused = element.ownerDocument.activeElement;
    // Older host webviews lack state-preserving moves; keep that capability at the DOM boundary.
    const moveBefore = Reflect.get(target, 'moveBefore');
    if (typeof moveBefore === 'function' && element.isConnected && target.isConnected)
        moveBefore.call(target, element, before);
    else
        target.insertBefore(element, before);
    if (focused instanceof HTMLElement &&
        element.contains(focused) &&
        element.ownerDocument.activeElement !== focused)
        focused.focus({ preventScroll: true });
}
