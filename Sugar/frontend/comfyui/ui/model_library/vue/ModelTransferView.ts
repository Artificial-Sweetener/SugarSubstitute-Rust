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
import { defineComponent, h } from 'vue';
import type { PropType } from 'vue';
import type { ModelTransferState } from '../ModelTransfer.js';
import { ModelTransferFill } from './ModelTransferFill.js';
import { useModelImage } from './ModelImage.js';
import { ModelImageElement } from './ModelImageElement.js';

/** Keep thumbnail failure and progress changes independent of the field's legal selected value. */
export const ModelTransferView = defineComponent({
  props: { transfer: { type: Object as PropType<ModelTransferState>, required: true } },
  setup(props) {
    const image = useModelImage(() => props.transfer.model.previewUrl);
    return () => {
      const { url } = image();
      return h('div', { class: 'sc-model-transfer-presentation' }, [
        url ? h(ModelImageElement, { class: 'sc-model-transfer-thumbnail', url }) : null,
        h(ModelTransferFill, { progress: props.transfer }),
        h('span', { class: 'sc-model-transfer-caption', 'aria-hidden': 'true' }, [
          h('span', null, 'Downloading'),
          h('span', null, props.transfer.model.name),
        ]),
      ]);
    };
  },
});
