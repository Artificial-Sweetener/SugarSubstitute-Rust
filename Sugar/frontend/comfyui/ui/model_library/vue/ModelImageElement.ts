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
/** Draw one model image so that a new address never shows an empty frame on its way in. */
import { defineComponent, h, ref, watch } from 'vue';

/** The class of an image that is still loading over the one on screen. */
export const INCOMING_MODEL_IMAGE = 'sc-model-image-incoming';

/**
 * The `<img>` of a model image.
 *
 * A decoded address is not yet a painted one: an `<img>` given a new `src` still loads and decodes
 * it for itself, and draws nothing meanwhile. So the image on screen stays, and the new one is laid
 * over it until that element can paint; then the old one goes. The element that loaded is the one
 * kept, so nothing loads twice. A new address that fails to load leaves the one on screen.
 */
export const ModelImageElement = defineComponent({
  name: 'SugarCubesModelImageElement',
  inheritAttrs: false,
  props: {
    url: { type: String, required: true },
  },
  setup(props, { attrs }) {
    const front = ref(props.url);
    const incoming = ref<string | null>(null);
    watch(
      () => props.url,
      (url) => {
        incoming.value = url === front.value ? null : url;
      },
    );
    const settle = (url: string, element: HTMLImageElement): void => {
      // `decode` resolves once the element itself can paint; a browser without it has loaded it.
      const ready = typeof element.decode === 'function' ? element.decode() : Promise.resolve();
      void ready.then(
        () => {
          if (incoming.value !== url) return;
          front.value = url;
          incoming.value = null;
        },
        () => {
          if (incoming.value === url) incoming.value = null;
        },
      );
    };
    const image = (url: string, extra: Record<string, unknown> = {}) =>
      h('img', { ...attrs, ...extra, key: url, src: url, alt: '', draggable: false });
    // Always a keyed list, so the element that loaded is the one kept when it takes the front.
    return () => {
      const next = incoming.value;
      if (next === null) return [image(front.value)];
      return [
        image(front.value),
        image(next, {
          class: [attrs.class, INCOMING_MODEL_IMAGE],
          onLoad: (event: Event) => settle(next, event.currentTarget as HTMLImageElement),
          onError: () => {
            if (incoming.value === next) incoming.value = null;
          },
        }),
      ];
    };
  },
});
