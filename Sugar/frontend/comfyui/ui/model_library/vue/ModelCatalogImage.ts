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
/** Retry failed prepared images on catalog publication while retaining loaded image elements. */
import { defineComponent, h, ref, watch } from 'vue';
import type { PropType } from 'vue';
import type { ModelThumbnail } from '../ModelCatalog.js';

/** Keep broken asset recovery shared between decorative banners and picker tiles. */
export const ModelCatalogImage = defineComponent({
  name: 'SugarCubeModelCatalogImage',
  inheritAttrs: false,
  props: {
    image: { type: Object as PropType<ModelThumbnail>, required: true },
    fallback: { type: Boolean, default: false },
    lazy: { type: Boolean, default: false },
  },
  setup(props, { attrs }) {
    const failed = ref(false);
    let attempt = 0;
    watch(
      () => props.image,
      (image, previous) => {
        if (image.url !== previous.url || failed.value) {
          attempt++;
          failed.value = false;
        }
      },
    );
    return () => {
      if (failed.value)
        return props.fallback ? h('span', { class: 'sc-model-no-image' }, 'No image') : null;
      const renderedAttempt = attempt;
      return h('img', {
        ...attrs,
        key: `${props.image.url}:${String(attempt)}`,
        src: props.image.url,
        alt: '',
        loading: props.lazy ? 'lazy' : 'eager',
        decoding: 'async',
        draggable: false,
        onError: () => {
          if (renderedAttempt === attempt) failed.value = true;
        },
      });
    };
  },
});
