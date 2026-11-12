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
/** Describe the browser workspace allocation independently of host DOM and workflow documents. */
/** Keep the output canvas usable however wide the user makes the editor. */
export const CANVAS_WORKSPACE_MINIMUM_CANVAS_WIDTH = 320;
/** Keep workflow controls usable however wide the user makes the canvas. */
export const CANVAS_WORKSPACE_MINIMUM_EDITOR_WIDTH = 360;
/** Below this workspace width the canvas becomes an explicitly opened drawer. */
export const CANVAS_WORKSPACE_DRAWER_BREAKPOINT = 960;
/**
 * Give the editor its stored width and the canvas everything else, so window growth widens the canvas.
 *
 * The editor width is clamped only for this allocation; the stored width returns once the workspace
 * widens again. Narrow workspaces overlay the canvas as a drawer that opens only on explicit request.
 *
 * @param defaultEditorWidth Docked editor width while the workflow has no stored width, in CSS pixels.
 * @throws RangeError when a dimension or width is negative or not finite.
 */
export function allocateCanvasWorkspace(width, height, preferences, drawerOpen, defaultEditorWidth) {
    if (![width, height, preferences.editorWidth, defaultEditorWidth].every((value) => Number.isFinite(value) && value >= 0))
        throw new RangeError('Canvas workspace dimensions are invalid.');
    const mode = width < CANVAS_WORKSPACE_DRAWER_BREAKPOINT ? 'drawer' : 'docked';
    const visible = preferences.visible && width > 0 && height > 0 && (mode === 'docked' || drawerOpen);
    const maximum = Math.max(0, Math.min(width, width - (mode === 'docked' ? CANVAS_WORKSPACE_MINIMUM_EDITOR_WIDTH : 24)));
    const minimum = Math.min(CANVAS_WORKSPACE_MINIMUM_CANVAS_WIDTH, maximum);
    const requested = preferences.editorWidth > 0
        ? width - preferences.editorWidth
        : mode === 'docked'
            ? width - defaultEditorWidth
            : width * 0.8;
    const paneWidth = visible ? Math.min(maximum, Math.max(minimum, requested)) : 0;
    const inset = mode === 'docked' ? paneWidth : 0;
    return Object.freeze({
        mode,
        visible,
        editorWidth: width - paneWidth,
        minimumWidth: minimum,
        maximumWidth: maximum,
        canvas: Object.freeze({ x: width - paneWidth, y: 0, width: paneWidth, height }),
        content: Object.freeze({ x: 0, y: 0, width: width - inset, height }),
    });
}
