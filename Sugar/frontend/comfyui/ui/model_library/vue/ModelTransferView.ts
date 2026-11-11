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
/** Paint the accepted model preview and compact transfer caption within the existing combo. */
import { defineComponent, h, ref, watch } from 'vue';
import type { PropType } from 'vue';
import type { ModelTransferState } from '../ModelTransfer.js';
import { ModelTransferFill } from './ModelTransferFill.js';

/** Keep thumbnail failure and progress changes independent of the field's legal selected value. */
export const ModelTransferView = defineComponent({
  props: { transfer: { type: Object as PropType<ModelTransferState>, required: true } },
  setup(props) {
    const failed = ref(false);
    watch(
      () => props.transfer.model.previewUrl,
      () => {
        failed.value = false;
      },
    );
    return () =>
      h('div', { class: 'sc-model-transfer-presentation' }, [
        props.transfer.model.previewUrl && !failed.value
          ? h('img', {
              class: 'sc-model-transfer-thumbnail',
              src: props.transfer.model.previewUrl,
              alt: '',
              draggable: false,
              decoding: 'async',
              onError: () => {
                failed.value = true;
              },
            })
          : null,
        h(ModelTransferFill, { progress: props.transfer }),
        h('span', { class: 'sc-model-transfer-caption', 'aria-hidden': 'true' }, [
          h('span', null, 'Downloading'),
          h('span', null, props.transfer.model.name),
        ]),
      ]);
  },
});
