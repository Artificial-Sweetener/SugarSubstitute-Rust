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
/** Render the Input/Output route as one canvas selector that opens a picker. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasRouteId, CanvasRouteState } from '../../domain/CanvasRoute.js';
import { CanvasAnchoredPicker } from './CanvasAnchoredPicker.js';

const ROUTES: readonly CanvasRouteId[] = ['input', 'output'];

/**
 * Show the current canvas and choose another one; absent while the workflow has no Input route.
 *
 * Its width fits every route label, so choosing a canvas never moves the controls after it.
 */
export const CanvasRoutePicker = defineComponent({
  name: 'CanvasRoutePicker',
  props: {
    state: { type: Object as PropType<CanvasRouteState>, required: true },
    choose: { type: Function as PropType<(route: CanvasRouteId) => void>, required: true },
  },
  /** Forward only a different, declared route. */
  setup(props) {
    /** Translate a picker row into a route change. */
    function choose(value: string): void {
      const route = ROUTES.find((candidate) => candidate === value);
      if (route && route !== props.state.route) props.choose(route);
    }
    return () =>
      props.state.inputAvailable
        ? h(
            'span',
            {
              class: 'sc-output-route',
              'data-sc-fixed-navigation': 'true',
              'data-route': props.state.route,
            },
            [
              h(CanvasAnchoredPicker, {
                label: props.state.labels.group,
                value: props.state.route,
                items: ROUTES.map((route) => ({ value: route, label: props.state.labels[route] })),
                stable: true,
                choose,
              }),
            ],
          )
        : null;
  },
});
