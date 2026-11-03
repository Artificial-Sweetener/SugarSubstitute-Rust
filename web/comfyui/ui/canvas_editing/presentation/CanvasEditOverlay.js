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
/** Compose the rail, tool options, history, contextual toolbar and coverage panel. */
import { defineComponent, h, onMounted, ref } from 'vue';
import { canvasEditChromeSuppressed } from '../domain/CanvasEditSurface.js';
import { canvasEditToolAvailable } from '../domain/CanvasEditTools.js';
import { CanvasBrushOptions } from './CanvasBrushOptions.js';
import { CanvasContextualToolbar } from './CanvasContextualToolbar.js';
import { CanvasCoverageControls } from './CanvasCoverageControls.js';
import { iconButton } from './CanvasEditPages.js';
import { CANVAS_EDIT_TEXT } from './CanvasEditText.js';
import { CanvasToolRail } from './CanvasToolRail.js';
/**
 * Render nothing until the host presents an editable document. While an exclusive editor owns
 * the canvas (coverage panel, selection modification, transform) the rail and options hide.
 */
export const CanvasEditOverlay = defineComponent({
    name: 'CanvasEditOverlay',
    props: {
        snapshot: { type: Object, default: null },
        controller: { type: Object, required: true },
        viewport: { type: Object, default: null },
        maskColor: { type: String, default: null },
    },
    setup(props) {
        return () => {
            const snapshot = props.snapshot;
            if (!snapshot)
                return null;
            const edit = props.controller;
            const surface = snapshot.surface;
            return [
                canvasEditChromeSuppressed(surface) ? null : dock(snapshot, edit, props.maskColor),
                props.viewport && surface.kind !== 'none' && surface.kind !== 'coverage'
                    ? h(CanvasContextualToolbar, { snapshot, controller: edit, viewport: props.viewport })
                    : null,
                surface.kind === 'coverage'
                    ? h(CanvasCoveragePanel, { snapshot, controller: edit, key: 'coverage' })
                    : null,
            ];
        };
    },
});
/** Top bar (document history, then the active tool's options) above the vertical rail. */
function dock(snapshot, edit, maskColor) {
    const state = snapshot.state;
    const tool = state.tool === 'brush' || state.tool === 'eraser' ? state.tool : null;
    return h('div', { class: 'sc-canvas-edit-dock' }, [
        h('div', { class: 'sc-canvas-edit-topbar' }, [
            h('div', {
                class: 'sc-canvas-edit-chrome sc-canvas-edit-row',
                role: 'group',
                'aria-label': CANVAS_EDIT_TEXT.history,
            }, [
                iconButton('undo', CANVAS_EDIT_TEXT.undo, 'undo', () => edit.execute({ type: 'history', action: 'undo' }), state.history.undo),
                iconButton('redo', CANVAS_EDIT_TEXT.redo, 'redo', () => edit.execute({ type: 'history', action: 'redo' }), state.history.redo),
            ]),
            tool && canvasEditToolAvailable(state, tool)
                ? h(CanvasBrushOptions, {
                    key: 'options',
                    tool,
                    options: state.options[tool],
                    color: maskColor,
                    change: (target, options) => edit.setBrushOptions(target, options),
                })
                : null,
        ]),
        h(CanvasToolRail, {
            slots: snapshot.slots,
            selectSlot: (slot) => edit.selectSlot(slot),
            selectTool: (tool) => edit.selectTool(tool),
        }),
    ]);
}
/**
 * The exclusive "Edit layer coverage" editor centred along the canvas bottom.
 *
 * It is a modal dialog: the native occlusion reporter treats an open `aria-modal` element as
 * blocking the whole canvas, so other canvas input stops while it is open. Esc closes it.
 */
const CanvasCoveragePanel = defineComponent({
    name: 'CanvasCoveragePanel',
    props: {
        snapshot: { type: Object, required: true },
        controller: { type: Object, required: true },
    },
    setup(props) {
        const element = ref(null);
        const applying = ref(false);
        onMounted(() => element.value?.querySelector('select')?.focus());
        return () => h('div', {
            ref: element,
            class: 'sc-canvas-edit-chrome sc-canvas-edit-coverage',
            role: 'dialog',
            'aria-modal': 'true',
            'aria-label': CANVAS_EDIT_TEXT.editLayerCoverage,
        }, [
            h(CanvasCoverageControls, {
                value: props.snapshot.coverage,
                disabled: applying.value,
                change: (value) => props.controller.updateCoverage(value),
            }),
            h('button', {
                type: 'button',
                class: 'sc-canvas-edit-primary',
                'data-canvas-action': 'apply',
                disabled: applying.value,
                onClick: () => {
                    applying.value = true;
                    void props.controller.applyCoverage().finally(() => {
                        applying.value = false;
                    });
                },
            }, CANVAS_EDIT_TEXT.apply),
            iconButton('close', CANVAS_EDIT_TEXT.cancel, 'cancel', () => props.controller.closeCoverage(), !applying.value),
        ]);
    },
});
