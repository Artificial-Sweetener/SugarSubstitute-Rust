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
/** Show one prepared model image, replacing it only once its replacement has loaded. */
import { defineComponent, h, ref, watch } from 'vue';
import type { PropType } from 'vue';
import type { ModelThumbnail } from '../ModelCatalog.js';

/**
 * The image every model surface shows: field banners, picker tiles and version rows.
 *
 * - A new image address loads out of sight; the image on screen changes only when it has loaded,
 *   so nothing blanks in between. A newer address supersedes one still loading.
 * - A replacement that fails leaves the image on screen. With nothing on screen, the fallback
 *   shows instead.
 * - A failed address is tried again only when the catalog publishes the model's image again.
 */
export const ModelCatalogImage = defineComponent({
  name: 'SugarCubeModelCatalogImage',
  inheritAttrs: false,
  props: {
    image: { type: Object as PropType<ModelThumbnail>, required: true },
    fallback: { type: Boolean, default: false },
    lazy: { type: Boolean, default: false },
  },
  setup(props, { attrs }) {
    /** The address on screen, once it has loaded. */
    const shown = ref<string | null>(null);
    const failed = ref(false);
    const attempt = ref(0);
    /** The attempt whose element is on screen, so that element survives later replacements. */
    let loadedAttempt = 0;
    watch(
      () => props.image,
      (image, previous) => {
        if (image.url !== previous.url || failed.value) {
          attempt.value++;
          failed.value = false;
        }
      },
    );
    return () => {
      const wanted = props.image.url;
      const renderedAttempt = attempt.value;
      const settle = (loaded: boolean): void => {
        if (renderedAttempt !== attempt.value || wanted !== props.image.url) return;
        if (!loaded) failed.value = true;
        else {
          if (shown.value === null) loadedAttempt = renderedAttempt;
          shown.value = wanted;
        }
      };
      if (shown.value === null && failed.value)
        return props.fallback ? h('span', { class: 'sc-model-no-image' }, 'No image') : null;
      const visible = h('img', {
        ...attrs,
        key: `visible:${String(shown.value === null ? renderedAttempt : loadedAttempt)}`,
        src: shown.value ?? wanted,
        alt: '',
        loading: props.lazy ? 'lazy' : 'eager',
        decoding: 'async',
        draggable: false,
        ...(shown.value === null
          ? { onLoad: () => settle(true), onError: () => settle(false) }
          : {}),
      });
      // Always the same two slots, so the image on screen is never remounted.
      return [
        visible,
        shown.value === null || shown.value === wanted || failed.value
          ? null
          : h('img', {
              key: `next:${wanted}:${String(renderedAttempt)}`,
              src: wanted,
              alt: '',
              'aria-hidden': 'true',
              'data-sc-model-image-loading': '',
              decoding: 'async',
              style: { display: 'none' },
              onLoad: () => settle(true),
              onError: () => settle(false),
            }),
      ];
    };
  },
});
