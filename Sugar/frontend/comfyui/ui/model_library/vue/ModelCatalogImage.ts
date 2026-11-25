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
/** Show one prepared model image through the presentation every model surface shares. */
import { defineComponent, h } from 'vue';
import type { PropType } from 'vue';
import type { ModelThumbnail } from '../ModelCatalog.js';
import { useModelImage } from './ModelImage.js';
import { ModelImageElement } from './ModelImageElement.js';

/**
 * A catalog image: a field banner or a wall tile. The image on screen is replaced only once its
 * replacement has been decoded; with nothing to show, the fallback shows instead when asked for.
 * The thumbnail record is what a failed address is remembered against, so the catalog publishing
 * the model's image again tries it again.
 */
export const ModelCatalogImage = defineComponent({
  name: 'SugarCubeModelCatalogImage',
  inheritAttrs: false,
  props: {
    image: { type: Object as PropType<ModelThumbnail>, required: true },
    fallback: { type: Boolean, default: false },
  },
  setup(props, { attrs }) {
    const image = useModelImage(
      () => props.image.url,
      () => props.image,
    );
    return () => {
      const { url, unavailable } = image();
      if (url) return h(ModelImageElement, { ...attrs, url });
      return unavailable && props.fallback
        ? h('span', { class: 'sc-model-no-image' }, 'No image')
        : null;
    };
  },
});
