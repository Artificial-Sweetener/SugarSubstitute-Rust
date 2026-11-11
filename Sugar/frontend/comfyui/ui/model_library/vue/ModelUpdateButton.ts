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
/** Present the reference update affordance without turning it into a model-selection gesture. */
import { defineComponent, h } from 'vue';

const updateIcon = new URL('../../../../icons/model/update.svg', import.meta.url).href;

/** Keep regular and context clicks distinct from the surrounding banner or tile. */
export const ModelUpdateButton = defineComponent({
  name: 'SugarCubesModelUpdateButton',
  emits: { open: () => true, context: (_event: MouseEvent) => true },
  setup(_props, { emit }) {
    return () =>
      h(
        'button',
        {
          type: 'button',
          class: 'sc-model-update',
          title: 'Update available — view versions',
          'aria-label': 'Update available — view versions',
          onPointerdown: (event: Event) => event.stopPropagation(),
          onClick: (event: Event) => {
            event.stopPropagation();
            emit('open');
          },
          onContextmenu: (event: MouseEvent) => {
            event.preventDefault();
            event.stopPropagation();
            emit('context', event);
          },
        },
        h('span', {
          'aria-hidden': true,
          style: { '--sc-model-update-icon': `url("${updateIcon}")` },
        }),
      );
  },
});
