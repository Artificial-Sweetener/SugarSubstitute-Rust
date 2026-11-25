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
/** Show a recommendation or version portrait through the presentation every model surface shares. */
import { defineComponent, h } from 'vue';
import type { PropType } from 'vue';
import { ModelBusyRing } from './ModelBusyRing.js';
import { useModelImage } from './ModelImage.js';

/** Keep image lifetime independent of selection, dialog state, and provider acquisition. */
export const ModelRecommendationPreview = defineComponent({
  name: 'SugarCubesModelRecommendationPreview',
  props: {
    url: { type: String as PropType<string | null>, default: null },
  },
  setup(props) {
    const image = useModelImage(() => props.url);
    return () => {
      const { url, loading, unavailable } = image();
      return h('span', { class: 'sc-model-recommendation-preview' }, [
        url
          ? h('img', { src: url, alt: '', draggable: false })
          : unavailable
            ? h('span', { class: 'sc-model-preview-unavailable' }, 'Preview unavailable')
            : null,
        loading
          ? h('span', { class: 'sc-model-preview-loading', role: 'status' }, [
              h(ModelBusyRing, { class: 'sc-model-preview-busy-ring', size: 34, strokeWidth: 4 }),
              h('span', { class: 'sc-model-preview-loading-label' }, 'Loading preview…'),
            ])
          : null,
      ]);
    };
  },
});
