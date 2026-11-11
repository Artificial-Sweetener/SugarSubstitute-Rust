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
/** Place independent source and batch choosers over the two images in reveal comparison. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasCommand, CanvasViewSnapshot } from '../../domain/CanvasViewPort.js';
import type { CanvasRouteId, CanvasRouteState } from '../../domain/CanvasRoute.js';
import { CanvasRoutePicker } from './CanvasRoutePicker.js';
import { CanvasSelectionPicker } from './CanvasSelectionPicker.js';
import { CanvasComparisonExit } from './CanvasComparisonExit.js';

/** Keep each reveal image's content navigation explicit and independently selectable; the first bar also chooses the canvas. */
export const CanvasComparisonNavigation = defineComponent({
  name: 'CanvasComparisonNavigation',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
    route: { type: Object as PropType<CanvasRouteState | null>, default: null },
    chooseRoute: { type: Function as PropType<(route: CanvasRouteId) => void>, required: true },
  },
  /** Bind both selectors to the same view authority used by browser and native overlays. */
  setup(props) {
    return () =>
      h('div', { class: 'sc-output-comparison-navigation' }, [
        h(
          'div',
          {
            class: 'sc-output-tabstrip sc-output-compare-first',
            'data-sc-canvas-backdrop': 'first',
            'data-sc-native-backdrop': props.snapshot.nativeBackdrop,
          },
          [
            h(CanvasComparisonExit, { send: props.send }),
            props.route
              ? h(CanvasRoutePicker, { state: props.route, choose: props.chooseRoute })
              : null,
            h(CanvasSelectionPicker, {
              sources: props.snapshot.sources,
              selection: props.snapshot.view.selection,
              label: 'First output',
              send: props.send,
            }),
          ],
        ),
        h(
          'div',
          {
            class: 'sc-output-tabstrip sc-output-compare-second',
            'data-sc-canvas-backdrop': 'second',
            'data-sc-native-backdrop': props.snapshot.nativeBackdrop,
          },
          [
            h(CanvasSelectionPicker, {
              sources: props.snapshot.sources,
              selection: props.snapshot.view.comparison?.secondary ?? null,
              label: 'Comparison output',
              secondary: true,
              send: props.send,
            }),
          ],
        ),
      ]);
  },
});
