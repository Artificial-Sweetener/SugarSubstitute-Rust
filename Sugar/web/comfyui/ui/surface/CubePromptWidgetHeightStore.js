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
/** Own transient Cube-face height requirements for graph-owned prompt widgets. */
const promptWidgetHeights = new WeakMap();
/** Record one normalized prompt allocation and report material changes. */
export function setCubePromptWidgetHeight(widget, height) {
    const normalized = Number.isFinite(height) ? Math.max(1, height) : 1;
    const previous = promptWidgetHeights.get(widget);
    if (previous !== undefined && Math.abs(previous - normalized) < 0.5)
        return false;
    promptWidgetHeights.set(widget, normalized);
    return true;
}
/** Read one prompt allocation without mutating the graph-owned widget. */
export function cubePromptWidgetHeight(widget) {
    return promptWidgetHeights.get(widget) ?? null;
}
/** Release prompt allocation when its Cube presentation ends. */
export function clearCubePromptWidgetHeight(widget) {
    promptWidgetHeights.delete(widget);
}
