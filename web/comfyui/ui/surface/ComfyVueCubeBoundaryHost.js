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
/** Position graph-owned Nodes 2.0 slots on the outer Cube edges. */
/** Own only structural positioning for Comfy's actual native slot elements. */
export class ComfyVueCubeBoundaryHost {
    #body;
    #requestSlotLayoutSync;
    #presentations = new Map();
    /** Bind one current native node body. */
    constructor(body, requestSlotLayoutSync = () => undefined) {
        this.#body = body;
        this.#requestSlotLayoutSync = requestSlotLayoutSync;
        this.reconcile();
    }
    /** Reapply markers after Comfy replaces slot descendants. */
    reconcile() {
        const row = [...this.#body.children].find((child) => child instanceof HTMLElement && child.querySelector('.lg-slot') !== null);
        if (!row)
            return;
        let changed = row.dataset.sugarcubeBoundaryRow === undefined;
        this.#remember(row);
        row.dataset.sugarcubeBoundaryRow = '';
        changed = this.#markSlots(row, 'input') || changed;
        changed = this.#markSlots(row, 'output') || changed;
        if (changed)
            this.#requestSlotLayoutSync();
    }
    /** Restore every Comfy-owned element exactly as it was mounted. */
    dispose() {
        for (const [element, presentation] of this.#presentations) {
            restoreAttribute(element, 'style', presentation.style);
            restoreDataset(element, 'sugarcubeBoundaryRow', presentation.boundaryRow);
            restoreDataset(element, 'sugarcubeBoundaryDirection', presentation.boundaryDirection);
            restoreDataset(element, 'sugarcubeBoundaryIndex', presentation.boundaryIndex);
        }
        this.#presentations.clear();
    }
    /** Mark one direction's native slots with finite evenly spaced positions. */
    #markSlots(row, direction) {
        const slots = [...row.querySelectorAll(`.lg-slot--${direction}`)];
        const outputPositions = direction === 'output' ? measureOutputTitlePositions(this.#body, row, slots.length) : [];
        let changed = false;
        for (const [index, slot] of slots.entries()) {
            this.#remember(slot);
            const boundaryIndex = String(index);
            const boundaryPosition = outputPositions[index] ?? `${String(((index + 1) * 100) / (slots.length + 1))}%`;
            changed =
                slot.dataset.sugarcubeBoundaryDirection !== direction ||
                    slot.dataset.sugarcubeBoundaryIndex !== boundaryIndex ||
                    slot.style.getPropertyValue('--sugarcube-boundary-position') !== boundaryPosition ||
                    changed;
            slot.dataset.sugarcubeBoundaryDirection = direction;
            slot.dataset.sugarcubeBoundaryIndex = boundaryIndex;
            slot.style.setProperty('--sugarcube-boundary-position', boundaryPosition);
        }
        return changed;
    }
    /** Retain host presentation once before applying Cube markers. */
    #remember(element) {
        if (this.#presentations.has(element))
            return;
        this.#presentations.set(element, {
            style: element.getAttribute('style'),
            boundaryRow: element.dataset.sugarcubeBoundaryRow,
            boundaryDirection: element.dataset.sugarcubeBoundaryDirection,
            boundaryIndex: element.dataset.sugarcubeBoundaryIndex,
        });
    }
}
/** Restore one nullable host attribute. */
function restoreAttribute(element, name, value) {
    if (value === null) {
        element.removeAttribute(name);
    }
    else {
        element.setAttribute(name, value);
    }
}
/** Restore one optional host dataset value. */
function restoreDataset(element, key, value) {
    if (value === undefined) {
        element.removeAttribute(DATASET_ATTRIBUTES[key]);
    }
    else {
        element.dataset[key] = value;
    }
}
const DATASET_ATTRIBUTES = {
    sugarcubeBoundaryRow: 'data-sugarcube-boundary-row',
    sugarcubeBoundaryDirection: 'data-sugarcube-boundary-direction',
    sugarcubeBoundaryIndex: 'data-sugarcube-boundary-index',
};
/** Measure preview-title centers in the body's unscaled CSS coordinate space. */
function measureOutputTitlePositions(body, row, count) {
    const titles = [...body.querySelectorAll('[data-cube-preview-output-title]')].slice(0, count);
    if (titles.length !== count || count === 0)
        return [];
    const bodyRect = body.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    const scale = body.offsetWidth > 0 ? bodyRect.width / body.offsetWidth : 1;
    const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
    return titles.map((title) => {
        const titleRect = title.getBoundingClientRect();
        const position = (titleRect.top + titleRect.height / 2 - rowRect.top) / safeScale;
        return `${String(Math.max(0, position))}px`;
    });
}
