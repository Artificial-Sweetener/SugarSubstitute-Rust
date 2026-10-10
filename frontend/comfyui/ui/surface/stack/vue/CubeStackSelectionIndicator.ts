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
/** Render one interruptible traveling marker rather than mounting selection marks on each row. */
import { defineComponent, h, type PropType } from 'vue';
import { CUBE_STACK_ROW_HEIGHT, CUBE_STACK_ROW_STRIDE } from '../CubeStackMetrics.js';

/** Animate one centered marker between the active card's settled row positions. */
export const CubeStackSelectionIndicator = defineComponent({
  props: { position: { type: Number as PropType<number | null>, default: null } },
  setup(props) {
    return () =>
      h('div', {
        class: 'sc-cube-stack__selection',
        'aria-hidden': 'true',
        'data-position': props.position ?? undefined,
        style: {
          opacity: props.position === null ? '0' : '1',
          transform: `translateY(${String((props.position ?? 0) * CUBE_STACK_ROW_STRIDE + (CUBE_STACK_ROW_HEIGHT - 16) / 2)}px)`,
        },
      });
  },
});
