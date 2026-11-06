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
/** Render Substitute's single numbered batch selector using anchored themed rows. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasCommand } from '../../domain/CanvasViewPort.js';
import type { CanvasSelection } from '../../domain/CanvasViewState.js';
import { CanvasAnchoredPicker } from './CanvasAnchoredPicker.js';

/** Translate numbered row choices into explicit shared-view batch commands. */
export const CanvasBatchPicker = defineComponent({
  name: 'CanvasBatchPicker',
  props: {
    count: { type: Number, required: true },
    grid: { type: Boolean, default: false },
    includeGrid: { type: Boolean, default: false },
    selection: { type: Object as PropType<CanvasSelection | null>, default: null },
    label: { type: String, required: true },
    secondary: { type: Boolean, default: false },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Show batch positions and the source overview for the active workflow. */
  setup(props) {
    return () => {
      const count = props.count;
      const selection = props.selection;
      if (count <= 1 || !selection) return null;
      return h(CanvasAnchoredPicker, {
        label: props.label,
        value: props.grid ? 'grid' : String(selection.batchIndex),
        text: props.grid ? '0' : String(selection.batchIndex + 1),
        centered: true,
        items: props.includeGrid
          ? [
              { value: 'grid', label: '0' },
              ...Array.from({ length: count }, (_, index) => ({
                value: String(count - index - 1),
                label: String(count - index),
              })),
            ]
          : Array.from({ length: count }, (_, index) => ({
              value: String(count - index - 1),
              label: String(count - index),
            })),
        choose: (value: string) => {
          if (value === 'grid' && props.includeGrid) {
            props.send({ type: 'overview', enabled: true });
            return;
          }
          const index = Number(value);
          if (!Number.isSafeInteger(index) || index < 0 || index >= count) return;
          props.send(
            props.secondary
              ? { type: 'select-comparison', selection: { ...selection, batchIndex: index } }
              : { type: 'batch', index },
          );
        },
      });
    };
  },
});
