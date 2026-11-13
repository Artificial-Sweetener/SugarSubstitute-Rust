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
/** Edit one coverage operation and its whole-pixel amount: "Expand by 4 Pixels". */
import { defineComponent, h, type PropType, type VNode } from 'vue';
import {
  CANVAS_COVERAGE_OPERATIONS,
  COVERAGE_PIXELS_MAX,
  COVERAGE_PIXELS_MIN,
  coverageOperation,
  coveragePixels,
  type CanvasCoverageModification,
} from '../domain/CanvasCoverageModification.js';
import { CANVAS_COVERAGE_OPERATION_LABELS, CANVAS_EDIT_TEXT } from './CanvasEditText.js';

/** Publish every operation or amount change; the owner previews it live. */
export const CanvasCoverageControls = defineComponent({
  name: 'CanvasCoverageControls',
  props: {
    value: { type: Object as PropType<CanvasCoverageModification>, required: true },
    disabled: { type: Boolean, default: false },
    change: {
      type: Function as PropType<(value: CanvasCoverageModification) => void>,
      required: true,
    },
  },
  setup(props) {
    return (): VNode =>
      h('div', { class: 'sc-canvas-edit-coverage-controls' }, [
        h(
          'select',
          {
            'aria-label': CANVAS_EDIT_TEXT.operation,
            'data-canvas-coverage': 'operation',
            value: props.value.operation,
            disabled: props.disabled,
            onChange: (event: Event) => {
              if (!(event.target instanceof HTMLSelectElement)) return;
              const operation = coverageOperation(event.target.value);
              if (operation && operation !== props.value.operation)
                props.change({ ...props.value, operation });
            },
          },
          CANVAS_COVERAGE_OPERATIONS.map((operation) =>
            h(
              'option',
              { value: operation, selected: operation === props.value.operation },
              CANVAS_COVERAGE_OPERATION_LABELS[operation],
            ),
          ),
        ),
        h('span', CANVAS_EDIT_TEXT.by),
        h('input', {
          type: 'number',
          inputmode: 'numeric',
          'aria-label': CANVAS_EDIT_TEXT.pixels,
          'data-canvas-coverage': 'pixels',
          min: COVERAGE_PIXELS_MIN,
          max: COVERAGE_PIXELS_MAX,
          step: 1,
          value: props.value.pixels,
          disabled: props.disabled,
          onInput: (event: Event) => {
            if (!(event.target instanceof HTMLInputElement)) return;
            const pixels = coveragePixels(event.target.value, props.value.pixels);
            if (pixels !== props.value.pixels) props.change({ ...props.value, pixels });
          },
          onChange: (event: Event) => {
            if (event.target instanceof HTMLInputElement)
              event.target.value = String(props.value.pixels);
          },
        }),
        h('span', CANVAS_EDIT_TEXT.pixels),
      ]);
  },
});
