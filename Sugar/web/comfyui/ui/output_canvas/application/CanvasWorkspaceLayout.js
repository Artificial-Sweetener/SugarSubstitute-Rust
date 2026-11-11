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
/** Own transient resize and drawer intent while persistence remains with the workflow view owner. */
import { allocateCanvasWorkspace, CANVAS_WORKSPACE_DRAWER_BREAKPOINT, } from '../domain/CanvasWorkspace.js';
/** Publish one allocation policy to browser regions without storing results or workflow state. */
export class CanvasWorkspaceLayout {
    defaultEditorWidth;
    #preferences = Object.freeze({ visible: true, editorWidth: 0 });
    #width = 0;
    #height = 0;
    #drawer = false;
    #draft = null;
    /**
     * Bind the editor width used by workflows that have no stored width.
     *
     * @param defaultEditorWidth Docked editor width in CSS pixels.
     * @throws RangeError when the width is negative or not finite.
     */
    constructor(defaultEditorWidth) {
        this.defaultEditorWidth = defaultEditorWidth;
        allocateCanvasWorkspace(0, 0, this.#preferences, false, defaultEditorWidth);
    }
    /** Accept only committed preferences and relinquish their superseded width preview. */
    preferences(preferences) {
        this.#allocate(this.#width, this.#height, preferences);
        this.#preferences = Object.freeze({ ...preferences });
        this.#draft = null;
        if (!preferences.visible)
            this.#drawer = false;
    }
    /** Collapse a drawer on breakpoint entry instead of opening it as a resize side effect. */
    bounds(width, height) {
        this.#allocate(width, height, this.#preferences);
        if (width < CANVAS_WORKSPACE_DRAWER_BREAKPOINT &&
            this.#width >= CANVAS_WORKSPACE_DRAWER_BREAKPOINT)
            this.#drawer = false;
        this.#width = width;
        this.#height = height;
    }
    /** Apply an explicitly accepted drawer action without changing the user's desktop width. */
    drawer(open) {
        this.#drawer = open;
    }
    /**
     * Preview a divider position, expressed as the canvas width the separator controls.
     *
     * The preview is held as an editor width so the committed result is the editor's, and is clamped
     * by the same rules as settled allocations.
     *
     * @returns The clamped canvas width in CSS pixels.
     */
    resize(canvasWidth) {
        if (!Number.isFinite(canvasWidth) || canvasWidth < 0)
            throw new RangeError('Canvas resize width is invalid.');
        // Zero selects the default width, so the narrowest requestable editor is one pixel.
        const editorWidth = Math.max(1, this.#width - canvasWidth);
        const allocation = this.#allocate(this.#width, this.#height, {
            ...this.#preferences,
            editorWidth,
        });
        this.#draft = allocation.editorWidth;
        return allocation.canvas.width;
    }
    /** Restore the accepted width when a captured resize is canceled or its attachment retires. */
    cancelResize() {
        this.#draft = null;
    }
    /** Read immutable rectangles for all consumers, including a zero-width hidden canvas. */
    read() {
        return this.#allocate(this.#width, this.#height, {
            ...this.#preferences,
            editorWidth: this.#draft ?? this.#preferences.editorWidth,
        });
    }
    /** Apply the authoritative policy with this layout's drawer state and default editor width. */
    #allocate(width, height, preferences) {
        return allocateCanvasWorkspace(width, height, preferences, this.#drawer, this.defaultEditorWidth);
    }
}
