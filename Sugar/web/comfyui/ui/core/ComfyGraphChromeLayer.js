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
 * Name ComfyUI's application chrome layer inside its graph container.
 *
 * ComfyUI mounts one direct child of `#graph-canvas-container` that holds its whole interface
 * chrome: the workflow tabs, the action bar, the side toolbar and panels, the bottom panel, and the
 * `.graph-canvas-panel` the graph's own toolbar lives in. It is the only child that contains the
 * graph canvas panel, and it may mount after the graph or be replaced while the page runs.
 */
/** The panel only ComfyUI's chrome layer holds. */
const GRAPH_CANVAS_PANEL = '.graph-canvas-panel';
/** Whether `element` is the chrome layer: a direct child of the graph container holding the panel. */
export function isGraphChromeLayer(element) {
    return (element.parentElement?.id === 'graph-canvas-container' &&
        element.querySelector(GRAPH_CANVAS_PANEL) !== null);
}
/** The chrome layer of the graph container `root`, when ComfyUI has mounted it. */
export function graphChromeLayer(root) {
    for (const child of root.children)
        if (child instanceof HTMLElement && isGraphChromeLayer(child))
            return child;
    return null;
}
