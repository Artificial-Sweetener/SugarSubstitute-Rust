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
/** Present the Brush and Eraser options as a compact header that expands into sliders. */
import { defineComponent, h, ref, watch } from 'vue';
import { BRUSH_LIMITS, withBrushOption } from '../domain/CanvasCoverageModification.js';
import { canvasEditIcon } from './CanvasEditIcons.js';
import { CANVAS_EDIT_TEXT } from './CanvasEditText.js';
let gradients = 0;
/**
 * Show the active tip in the active mask colour (white for the Eraser) and, once expanded, Size,
 * Hardness and Opacity sliders. A drag shows its own value until the host reports it.
 */
export const CanvasBrushOptions = defineComponent({
    name: 'CanvasBrushOptions',
    props: {
        tool: { type: String, required: true },
        options: { type: Object, required: true },
        /** CSS colour of the active mask; the theme accent stands in when the host names none. */
        color: { type: String, default: null },
        change: {
            type: Function,
            required: true,
        },
    },
    setup(props) {
        const expanded = ref(false);
        const draft = ref(null);
        const gradient = `sc-canvas-edit-tip-${++gradients}`;
        watch(() => props.tool, () => {
            draft.value = null;
        });
        // Intermediate replies of a coalesced drag must not pull the slider back.
        watch(() => props.options, (options) => {
            const pending = draft.value;
            if (pending &&
                pending.size === options.size &&
                pending.hardness === options.hardness &&
                pending.opacity === options.opacity)
                draft.value = null;
        });
        /** Send one slider's value and keep showing it until the host's state arrives. */
        function update(field, value) {
            const next = withBrushOption(draft.value ?? props.options, field, value);
            draft.value = next;
            props.change(props.tool, next);
        }
        function slider(field, label, value) {
            const current = (draft.value ?? props.options)[field];
            return [
                h('label', { for: `${gradient}-${field}` }, label),
                h('input', {
                    id: `${gradient}-${field}`,
                    type: 'range',
                    'data-canvas-option': field,
                    min: BRUSH_LIMITS[field].min,
                    max: BRUSH_LIMITS[field].max,
                    step: 1,
                    value: current,
                    onInput: (event) => {
                        if (event.target instanceof HTMLInputElement)
                            update(field, event.target.valueAsNumber);
                    },
                }),
                h('span', { 'data-canvas-option-value': field }, value),
            ];
        }
        return () => {
            const options = draft.value ?? props.options;
            const tint = props.tool === 'eraser'
                ? '#ffffff'
                : (props.color ?? 'var(--p-primary-color,var(--ring,#df258e))');
            return h('div', {
                class: 'sc-canvas-edit-chrome sc-canvas-edit-options',
                'data-canvas-options': props.tool,
            }, [
                h('div', { class: 'sc-canvas-edit-options-header' }, [
                    h('button', {
                        type: 'button',
                        'aria-expanded': String(expanded.value),
                        onClick: () => {
                            expanded.value = true;
                        },
                    }, [tipPreview(gradient, options, tint), CANVAS_EDIT_TEXT.brushSettings]),
                    expanded.value
                        ? h('button', {
                            type: 'button',
                            class: 'sc-canvas-edit-icon-button',
                            title: CANVAS_EDIT_TEXT.close,
                            'aria-label': CANVAS_EDIT_TEXT.close,
                            onClick: () => {
                                expanded.value = false;
                            },
                        }, [canvasEditIcon('close')])
                        : null,
                ]),
                expanded.value
                    ? h('div', { class: 'sc-canvas-edit-sliders' }, [
                        ...slider('size', CANVAS_EDIT_TEXT.size, CANVAS_EDIT_TEXT.pixelValue(options.size)),
                        ...slider('hardness', CANVAS_EDIT_TEXT.hardness, CANVAS_EDIT_TEXT.percentValue(options.hardness)),
                        ...slider('opacity', CANVAS_EDIT_TEXT.opacity, CANVAS_EDIT_TEXT.percentValue(options.opacity)),
                    ])
                    : null,
            ]);
        };
    },
});
/**
 * Draw the tip at a fixed 16 px diameter: solid to its hardness radius, then fading to the edge,
 * at its opacity. Size is shown as text because the preview, like the reference, is fixed.
 */
function tipPreview(id, options, color) {
    const hardness = `${options.hardness}%`;
    return h('svg', {
        class: 'sc-canvas-edit-tip',
        viewBox: '0 0 20 20',
        'aria-hidden': 'true',
        'data-hardness': options.hardness,
        'data-opacity': options.opacity,
        'data-color': color,
    }, [
        h('defs', [
            h('radialGradient', { id }, [
                h('stop', { offset: '0%', style: { stopColor: color, stopOpacity: 1 } }),
                h('stop', { offset: hardness, style: { stopColor: color, stopOpacity: 1 } }),
                h('stop', { offset: '100%', style: { stopColor: color, stopOpacity: 0 } }),
            ]),
        ]),
        h('circle', {
            cx: 10,
            cy: 10,
            r: 8,
            fill: `url(#${id})`,
            'fill-opacity': options.opacity / 100,
        }),
    ]);
}
