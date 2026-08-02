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
/** Adopt Cube-owned controls into the exact native Nodes 2 card header row. */
/** Own one accessory's relationship with Comfy's replaceable Vue header DOM. */
export class ComfyVueNodeHeaderAccessoryHost {
    #accessory;
    #header = null;
    /** Bind one renderer-neutral accessory element. */
    constructor(accessory) {
        this.#accessory = accessory;
    }
    /** Move the accessory into the current native header's in-flow trailing row. */
    reconcile(nativeRoot, node) {
        const header = findNativeNodeHeader(nativeRoot, node);
        if (!header)
            return false;
        const row = findNativeNodeHeaderRow(header);
        if (!row)
            return false;
        if (this.#header && this.#header !== header) {
            delete this.#header.dataset.sugarcubeCardHeaderAccessory;
        }
        this.#header = header;
        header.dataset.sugarcubeCardHeaderAccessory = '';
        if (this.#accessory.parentElement !== row)
            row.append(this.#accessory);
        return true;
    }
    /** Release the accessory and every marker owned by this adapter. */
    dispose() {
        if (this.#header)
            delete this.#header.dataset.sugarcubeCardHeaderAccessory;
        this.#accessory.remove();
        this.#header = null;
    }
}
/** Locate one exact card header without matching projected descendant cards. */
function findNativeNodeHeader(nativeRoot, node) {
    const expectedTestId = `node-header-${String(node.id ?? '')}`;
    for (const candidate of nativeRoot.querySelectorAll('[data-testid^="node-header-"]')) {
        if (candidate.getAttribute('data-testid') === expectedTestId)
            return candidate;
    }
    return null;
}
/** Resolve Comfy's stable title-and-badges flex row inside the native header. */
function findNativeNodeHeaderRow(header) {
    const title = header.querySelector('[data-testid="node-title"]');
    const semanticRow = title?.parentElement?.parentElement;
    if (semanticRow instanceof HTMLElement && semanticRow.parentElement === header) {
        return semanticRow;
    }
    const firstChild = header.firstElementChild;
    return firstChild instanceof HTMLElement ? firstChild : null;
}
