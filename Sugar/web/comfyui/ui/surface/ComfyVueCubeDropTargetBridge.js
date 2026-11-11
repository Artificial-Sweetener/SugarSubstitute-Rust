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
/** Preserve projected Nodes 2 cards as Comfy's selected native drop targets. */
/** Prevent the enclosing Cube component from replacing one nested native drop target. */
export class ComfyVueCubeDropTargetBridge {
    #face;
    #outerNode;
    #nodes = new Map();
    #setDropTarget;
    /** Bind only the propagation seam created by nesting native LGraphNode components. */
    constructor(options) {
        this.#face = options.face;
        this.#outerNode = options.face.closest('.lg-node');
        this.#setDropTarget = options.setDropTarget;
        for (const node of options.nodes)
            this.#nodes.set(String(node.id ?? ''), node);
        this.#face.addEventListener('dragover', this.#handleDragOver);
        this.#outerNode?.addEventListener('drop', this.#handleOuterDrop);
    }
    /** Release listeners without changing any Comfy node callbacks. */
    dispose() {
        this.#face.removeEventListener('dragover', this.#handleDragOver);
        this.#outerNode?.removeEventListener('drop', this.#handleOuterDrop);
    }
    /** Keep an accepted internal dragover from also activating the enclosing Cube. */
    #handleDragOver = (event) => {
        if (!this.#resolveTarget(event))
            return;
        event.stopPropagation();
    };
    /** Restore the internal target after Comfy's enclosing LGraphNode handles drop. */
    #handleOuterDrop = (event) => {
        const target = this.#resolveTarget(event);
        if (target)
            this.#setDropTarget(target);
    };
    /** Resolve one nested card from the stable node identifier authored by SugarCubes. */
    #resolveTarget(event) {
        const eventTarget = event.target;
        if (!(eventTarget instanceof Element))
            return null;
        const card = eventTarget.closest('[data-cube-node-id]');
        if (!card || !this.#face.contains(card))
            return null;
        return this.#nodes.get(card.dataset.cubeNodeId ?? '') ?? null;
    }
}
