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
/** Resolve the vendored editing artwork, tinted by the current text colour through a CSS mask. */
import { h } from 'vue';
/** Microsoft Fluent UI System Icons, copied verbatim at the pinned upstream revision. */
const FLUENT = [
    'pan-zoom',
    'move',
    'brush',
    'eraser',
    'select-rectangle',
    'select-lasso',
    'transform',
    'shared-edges',
    'undo',
    'redo',
    'rotate-left',
    'rotate-right',
    'flip-horizontal',
    'flip-vertical',
];
/** SugarSubstitute's Fluent-derived shape artwork and QFluent's dialog artwork. */
const SUBSTITUTE = [
    'select-ellipse',
    'mask-rectangle',
    'mask-ellipse',
    'mask-lasso',
    'deselect',
    'apply',
    'close',
    'modify',
];
const URLS = new Map([
    ...FLUENT.map((name) => [
        name,
        new URL(`../../../../icons/canvas-editing/${name}.svg`, import.meta.url).href,
    ]),
    ...SUBSTITUTE.map((name) => [name, new URL(`../../../../canvas-editing/${name}.svg`, import.meta.url).href]),
]);
/** Each tool's artwork, as the reference catalogue assigns it. */
export const CANVAS_TOOL_ICONS = Object.freeze({
    'pan-zoom': 'pan-zoom',
    move: 'move',
    brush: 'brush',
    eraser: 'eraser',
    'select-rectangle': 'select-rectangle',
    'select-ellipse': 'select-ellipse',
    'select-lasso': 'select-lasso',
    'mask-rectangle': 'mask-rectangle',
    'mask-ellipse': 'mask-ellipse',
    'mask-lasso': 'mask-lasso',
    transform: 'transform',
    'shared-edges': 'shared-edges',
});
/** Express one icon as the CSS mask image its element is tinted through. */
function maskImage(name) {
    return `url("${URLS.get(name) ?? ''}")`;
}
/** Render one decorative icon; its accessible name belongs to the owning control. */
export function canvasEditIcon(name) {
    return h('span', {
        class: 'sc-canvas-edit-icon',
        'data-icon': name,
        'aria-hidden': 'true',
        style: { '--sc-canvas-edit-icon': maskImage(name) },
    });
}
/** Build the same decorative icon for menu rows, which the shared menu owner renders. */
export function createCanvasEditIcon(document, name) {
    const icon = document.createElement('span');
    icon.className = 'sc-canvas-edit-icon';
    icon.setAttribute('data-icon', name);
    icon.setAttribute('aria-hidden', 'true');
    icon.style.setProperty('--sc-canvas-edit-icon', maskImage(name));
    return icon;
}
