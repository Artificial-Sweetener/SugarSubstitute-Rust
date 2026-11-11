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
/** Draw the reference indeterminate arc without a background track or layout animation. */
import { defineComponent, h } from 'vue';

/** Share one bounded arc geometry across model status and portrait surfaces. */
export const ModelBusyRing = defineComponent({
  name: 'SugarCubesModelBusyRing',
  props: {
    size: { type: Number, default: 24 },
    strokeWidth: { type: Number, default: 6 },
  },
  setup(props) {
    return () =>
      h(
        'svg',
        {
          class: 'sc-model-busy-ring',
          viewBox: `0 0 ${props.size} ${props.size}`,
          style: { width: `${props.size}px`, height: `${props.size}px` },
          'aria-hidden': 'true',
          focusable: 'false',
        },
        [
          h('circle', {
            cx: props.size / 2,
            cy: props.size / 2,
            r: (props.size - props.strokeWidth) / 2,
            fill: 'none',
            stroke: 'currentColor',
            'stroke-width': props.strokeWidth,
            'stroke-linecap': 'round',
            pathLength: 360,
            'stroke-dasharray': '105 255',
          }),
        ],
      );
  },
});
