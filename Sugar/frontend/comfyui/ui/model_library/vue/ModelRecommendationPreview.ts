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
/** Settle recommendation previews consistently across discovery and version portraits. */
import { defineComponent, h, ref, watch } from 'vue';
import type { PropType } from 'vue';
import { ModelBusyRing } from './ModelBusyRing.js';

/** Keep image lifetime independent of selection, dialog state, and provider acquisition. */
export const ModelRecommendationPreview = defineComponent({
  name: 'SugarCubesModelRecommendationPreview',
  props: {
    url: { type: String as PropType<string | null>, default: null },
  },
  setup(props) {
    const status = ref<'loading' | 'ready' | 'unavailable'>('unavailable');
    let revision = 0;
    watch(
      () => props.url,
      (url) => {
        revision++;
        status.value = url ? 'loading' : 'unavailable';
      },
      { immediate: true, flush: 'sync' },
    );
    return () => {
      const url = props.url;
      const renderedRevision = revision;
      return h('span', { class: 'sc-model-recommendation-preview' }, [
        url && status.value !== 'unavailable'
          ? h('img', {
              key: url,
              src: url,
              alt: '',
              draggable: false,
              decoding: 'async',
              onLoad: () => {
                if (renderedRevision === revision && status.value === 'loading')
                  status.value = 'ready';
              },
              onError: () => {
                if (renderedRevision === revision && status.value === 'loading')
                  status.value = 'unavailable';
              },
            })
          : h('span', { class: 'sc-model-preview-unavailable' }, 'Preview unavailable'),
        status.value === 'loading'
          ? h('span', { class: 'sc-model-preview-loading', role: 'status' }, [
              h(ModelBusyRing, { class: 'sc-model-preview-busy-ring', size: 34, strokeWidth: 4 }),
              h('span', { class: 'sc-model-preview-loading-label' }, 'Loading preview…'),
            ])
          : null,
      ]);
    };
  },
});
