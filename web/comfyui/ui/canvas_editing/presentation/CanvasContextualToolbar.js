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
/** Float the contextual toolbar beside the selection, or along the canvas bottom without one. */
import { defineComponent, h, onMounted, onUpdated, ref, watch, } from 'vue';
import { CanvasToolbarPlacement, canvasEditSafeArea, } from '../domain/CanvasToolbarPlacement.js';
import { modifyPage, selectionPage, sessionPage, transformPage } from './CanvasEditPages.js';
import { CANVAS_EDIT_TEXT, CANVAS_TOOL_LABELS } from './CanvasEditText.js';
/**
 * Render the page for the current surface and place it after layout.
 *
 * Selection bounds arrive in surface CSS pixels; offsetting them by the viewport origin maps them
 * into the overlay. The placement owner keeps its above/below choice while bounds follow pan and
 * zoom, and starts afresh when the page changes.
 */
export const CanvasContextualToolbar = defineComponent({
    name: 'CanvasContextualToolbar',
    props: {
        snapshot: { type: Object, required: true },
        controller: { type: Object, required: true },
        viewport: { type: Object, required: true },
    },
    setup(props) {
        const element = ref(null);
        const placement = new CanvasToolbarPlacement();
        watch(() => props.snapshot.surface.kind, () => placement.reset());
        /** Position from the measured toolbar size; layout must precede placement. */
        function place() {
            const toolbar = element.value;
            if (!toolbar)
                return;
            const viewport = props.viewport;
            const bounds = props.snapshot.state.selection?.bounds;
            const context = bounds
                ? {
                    left: viewport.left + bounds.left,
                    top: viewport.top + bounds.top,
                    width: bounds.width,
                    height: bounds.height,
                }
                : null;
            const point = placement.position(context, { width: toolbar.offsetWidth, height: toolbar.offsetHeight }, canvasEditSafeArea(viewport), viewport.density);
            toolbar.style.transform = `translate(${point.x}px, ${point.y}px)`;
            toolbar.dataset.canvasPlacement = `${point.x},${point.y}`;
        }
        onMounted(place);
        onUpdated(place);
        return () => {
            const children = toolbarPage(props.snapshot, props.controller);
            if (!children)
                return null;
            return h('div', {
                ref: element,
                class: 'sc-canvas-edit-chrome sc-canvas-edit-row sc-canvas-edit-toolbar',
                role: 'toolbar',
                'aria-label': toolbarLabel(props.snapshot),
                'data-canvas-surface': props.snapshot.surface.kind,
            }, children);
        };
    },
});
function toolbarLabel(snapshot) {
    const surface = snapshot.surface;
    if (surface.kind === 'modify')
        return CANVAS_EDIT_TEXT.modifySelection;
    if (surface.kind === 'transform')
        return CANVAS_EDIT_TEXT.transform;
    if (surface.kind === 'session')
        return CANVAS_TOOL_LABELS[surface.tool];
    return CANVAS_EDIT_TEXT.selectionActions;
}
function toolbarPage(snapshot, edit) {
    const surface = snapshot.surface;
    switch (surface.kind) {
        case 'selection':
            return selectionPage(snapshot, edit);
        case 'modify':
            return modifyPage(snapshot, edit);
        case 'transform':
            return transformPage(snapshot, edit, surface.target);
        case 'session':
            return sessionPage(snapshot, edit, surface.tool);
        default:
            return null;
    }
}
