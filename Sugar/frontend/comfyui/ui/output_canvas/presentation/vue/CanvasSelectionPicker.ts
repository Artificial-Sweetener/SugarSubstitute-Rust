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
/** Render compact source and batch choices for normal navigation and either comparison image. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasCommand, CanvasSourceSummary } from '../../domain/CanvasViewPort.js';
import type { CanvasSelection } from '../../domain/CanvasViewState.js';
import { CanvasAnchoredPicker } from './CanvasAnchoredPicker.js';
import { CanvasBatchPicker } from './CanvasBatchPicker.js';

/** Render literal source labels and contextual batch choices from the shared snapshot. */
export const CanvasSelectionPicker = defineComponent({
  name: 'CanvasSelectionPicker',
  props: {
    sources: { type: Array as PropType<readonly CanvasSourceSummary[]>, required: true },
    selection: { type: Object as PropType<CanvasSelection | null>, default: null },
    label: { type: String, required: true },
    secondary: { type: Boolean, default: false },
    preview: { type: Boolean, default: false },
    includeBatch: { type: Boolean, default: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Send selections through the shared authority without choosing results in presentation code. */
  setup(props) {
    /** Read only source identities declared by the current immutable snapshot. */
    function chooseSource(value: string): void {
      const source = props.sources.find((source) => source.id === value);
      if (!source) return;
      props.send({
        type: props.secondary ? 'select-comparison' : props.preview ? 'select-source' : 'select',
        selection: { sourceId: source.id, batchIndex: props.selection?.batchIndex ?? 0 },
      });
    }
    return () => {
      return h('div', { class: 'sc-output-selection-picker' }, [
        props.includeBatch
          ? h(CanvasBatchPicker, {
              count: Math.max(0, ...props.sources.map((value) => value.batchCount)),
              selection: props.selection,
              label: `${props.label} image`,
              secondary: props.secondary,
              send: props.send,
            })
          : null,
        h(CanvasAnchoredPicker, {
          label: props.label,
          value: props.selection?.sourceId ?? '',
          items: props.sources.map((source) => ({ value: source.id, label: source.label })),
          choose: chooseSource,
        }),
      ]);
    };
  },
});
