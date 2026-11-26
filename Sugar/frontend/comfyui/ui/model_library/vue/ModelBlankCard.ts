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
/** Say, where a model's image goes, why there is none. */
import { defineComponent, h } from 'vue';
import type { PropType, VNode } from 'vue';
import type { ModelImageState } from '../ModelCatalog.js';

const eyeOffIcon = new URL('../../../../icons/model/eye-off.svg', import.meta.url).href;

/** What a person is told when the thumbnail safety setting excludes a model's image. */
export const FILTERED_THUMBNAIL_REASON = 'Hidden by your thumbnail safety setting';

/**
 * A card with no image, in the place the image would take.
 *
 * - Waiting for a lookup or a fetch: the card flashes, with no text.
 * - The same after a failure: it flashes and says "Retrying…".
 * - Excluded by the safety setting: a crossed-out eye, explained by its tooltip alone.
 * - No provider knows the file: "Not Found".
 * - Known, with no image: "No Thumbnail".
 */
export const ModelBlankCard = defineComponent({
  name: 'SugarCubesModelBlankCard',
  props: {
    state: { type: String as PropType<ModelImageState>, required: true },
  },
  setup(props) {
    return (): VNode => {
      if (props.state === 'pending' || props.state === 'retrying')
        return h(
          'span',
          {
            class: 'sc-model-blank sc-model-blank-flash',
            role: 'status',
            'aria-label': props.state === 'retrying' ? 'Retrying' : 'Loading',
          },
          props.state === 'retrying'
            ? h('span', { class: 'sc-model-blank-text' }, 'Retrying…')
            : [],
        );
      if (props.state === 'filtered')
        return h(
          'span',
          { class: 'sc-model-blank', title: FILTERED_THUMBNAIL_REASON },
          h('span', {
            class: 'sc-model-blank-icon',
            role: 'img',
            'aria-label': FILTERED_THUMBNAIL_REASON,
            style: { '--sc-model-blank-icon': `url("${eyeOffIcon}")` },
          }),
        );
      return h(
        'span',
        { class: 'sc-model-blank' },
        h(
          'span',
          { class: 'sc-model-blank-text' },
          props.state === 'not_found' ? 'Not Found' : 'No Thumbnail',
        ),
      );
    };
  },
});
