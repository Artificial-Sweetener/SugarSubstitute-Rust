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
/** Observe host renderer presence changes relevant to Cube presentation. */
/** Wake presentation when renderer chrome or an expected native Cube root changes. */
export class ComfyRendererPresenceObserver {
    #observer;
    /** Observe the native document without reacting to Cube face mutations. */
    constructor(options) {
        const root = options.document.body ?? options.document.documentElement;
        this.#observer = new MutationObserver((records) => {
            if (!records.some((record) => recordChangesRendererClass(record, root) ||
                recordChangesOwnedRoot(record, options.ownsNodeId))) {
                return;
            }
            options.onPresenceChange();
        });
        this.#observer.observe(root, {
            attributes: true,
            attributeFilter: ['class'],
            childList: true,
            subtree: true,
        });
    }
    /** Release native DOM observation. */
    dispose() {
        this.#observer.disconnect();
    }
}
/** Detect Comfy's renderer-owned body class transition without observing face styling. */
function recordChangesRendererClass(record, root) {
    return record.type === 'attributes' && record.target === root && record.attributeName === 'class';
}
/** Detect a catalogued native root in one mutation record. */
function recordChangesOwnedRoot(record, ownsNodeId) {
    return ([...record.addedNodes].some((node) => containsOwnedRoot(node, ownsNodeId)) ||
        [...record.removedNodes].some((node) => containsOwnedRoot(node, ownsNodeId)));
}
/** Inspect only native node roots in the changed subtree. */
function containsOwnedRoot(node, ownsNodeId) {
    if (!(node instanceof Element))
        return false;
    if (isOwnedRoot(node, ownsNodeId))
        return true;
    for (const root of node.querySelectorAll('.lg-node[data-node-id]')) {
        if (isOwnedRoot(root, ownsNodeId))
            return true;
    }
    return false;
}
/** Match one real native root against the current Cube catalog. */
function isOwnedRoot(element, ownsNodeId) {
    if (!element.matches('.lg-node[data-node-id]'))
        return false;
    const nodeId = element.getAttribute('data-node-id')?.trim() ?? '';
    return nodeId !== '' && ownsNodeId(nodeId);
}
