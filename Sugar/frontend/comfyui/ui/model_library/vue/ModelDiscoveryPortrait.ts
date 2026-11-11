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
/** Settle each reviewed preview independently without changing its explicit selection. */
import { defineComponent, h } from 'vue';
import type { PropType } from 'vue';
import { ModelRecommendationPreview } from './ModelRecommendationPreview.js';

/** Project the reference portrait's loading, unavailable, and selectable states. */
export const ModelDiscoveryPortrait = defineComponent({
  name: 'SugarCubesModelDiscoveryPortrait',
  props: {
    previewUrl: { type: String as PropType<string | null>, default: null },
    title: { type: String, required: true },
    selected: { type: Boolean, required: true },
    disabled: { type: Boolean, required: true },
  },
  emits: { select: () => true },
  setup(props, { emit }) {
    return () => {
      return h(
        'button',
        {
          type: 'button',
          class: ['sc-model-discovery-portrait', { 'is-selected': props.selected }],
          role: 'checkbox',
          'aria-checked': props.selected,
          'aria-label': props.title,
          disabled: props.disabled,
          onClick: () => emit('select'),
        },
        [
          h(ModelRecommendationPreview, { url: props.previewUrl }),
          h(
            'span',
            { class: 'sc-model-discovery-caption' },
            props.title
              .split('|')
              .flatMap((part, index, parts) =>
                index === parts.length - 1 ? [part] : [part + '|', h('wbr')],
              ),
          ),
          h(
            'span',
            { class: 'sc-model-discovery-check', 'aria-hidden': 'true' },
            props.selected ? '✓' : '',
          ),
        ],
      );
    };
  },
});
