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
/** Let a file dragged from the desktop be dropped on any SugarCubes surface, not only the graph. */
/**
 * Accept a dragged file wherever nothing else has.
 *
 * ComfyUI opens a dropped workflow from a handler on the document, but a browser delivers a drop
 * only where the drag was accepted, and ComfyUI accepts it on its graph canvas and its nodes alone.
 * The masonry, the frame and every other SugarCubes surface cover that canvas, so a file dropped on
 * them was refused before ComfyUI could see it. This owner accepts the drag there and does nothing
 * else: the drop still reaches ComfyUI's own handler, which decides what the file is.
 */
export class WorkflowFileDragAcceptor {
    document;
    #onDragOver = (event) => this.#accept(event);
    #active = false;
    constructor(document) {
        this.document = document;
    }
    /** Start accepting; listening after every other owner, so theirs decide first. */
    setup() {
        if (this.#active)
            return;
        this.#active = true;
        this.document.addEventListener('dragover', this.#onDragOver);
    }
    /** Stop accepting when the host lifecycle ends. */
    dispose() {
        if (!this.#active)
            return;
        this.#active = false;
        this.document.removeEventListener('dragover', this.#onDragOver);
    }
    /** Accept only a drag that carries files and that no owner nearer the pointer has answered. */
    #accept(event) {
        if (event.defaultPrevented)
            return;
        const transfer = event.dataTransfer;
        if (!transfer || ![...transfer.types].includes('Files'))
            return;
        event.preventDefault();
        transfer.dropEffect = 'copy';
    }
}
