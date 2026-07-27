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
/** Own Cube-only prompt textarea growth without changing Comfy's normal node policy. */
const promptWidgetHeights = new WeakMap();
/** Fit one prompt textarea to its full content and suppress its internal scrollbar. */
export function fitCubeFacePromptTextarea(textarea) {
    textarea.style.setProperty('height', 'auto', 'important');
    textarea.style.setProperty('overflow-y', 'hidden', 'important');
    const contentHeight = Math.max(textarea.scrollHeight, textarea.clientHeight, textarea.offsetHeight, 1);
    textarea.style.setProperty('height', `${String(contentHeight)}px`, 'important');
    return contentHeight;
}
/** Persist one Cube-face-only widget minimum without altering the graph-owned widget shape. */
export function setCubeFacePromptWidgetHeight(widget, height) {
    const normalized = Number.isFinite(height) ? Math.max(1, height) : 1;
    const previous = promptWidgetHeights.get(widget);
    if (previous !== undefined && Math.abs(previous - normalized) < 0.5)
        return false;
    promptWidgetHeights.set(widget, normalized);
    return true;
}
/** Read the currently required Cube-face height for one grown prompt widget. */
export function cubeFacePromptWidgetHeight(widget) {
    return promptWidgetHeights.get(widget) ?? null;
}
/** Release transient face sizing when the prompt leaves the Cube surface. */
export function clearCubeFacePromptWidgetHeight(widget) {
    promptWidgetHeights.delete(widget);
}
