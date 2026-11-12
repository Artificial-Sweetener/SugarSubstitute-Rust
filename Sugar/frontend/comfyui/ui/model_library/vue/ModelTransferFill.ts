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
/** Render bounded download progress as paint within the existing model field. */
import { defineComponent, h } from 'vue';
import type { PropType } from 'vue';
import type { ModelTransferProgress } from '../ModelTransfer.js';

/** Keep determinate and indeterminate progress independent of picker layout and selection. */
export const ModelTransferFill = defineComponent({
  props: { progress: { type: Object as PropType<ModelTransferProgress>, required: true } },
  setup(props) {
    return () =>
      h('span', {
        class: [
          'sc-model-transfer-fill',
          { 'sc-model-transfer-indeterminate': props.progress.percentage === null },
        ],
        role: 'progressbar',
        'aria-label': 'Model download',
        'aria-valuemin': 0,
        'aria-valuemax': 100,
        'aria-valuenow': props.progress.percentage ?? undefined,
        style:
          props.progress.percentage === null
            ? undefined
            : { width: `${String(props.progress.percentage)}%` },
      });
  },
});
