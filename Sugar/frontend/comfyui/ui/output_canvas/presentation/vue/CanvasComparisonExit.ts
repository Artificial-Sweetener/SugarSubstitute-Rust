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
/** Keep comparison exit visible independently of image or host readiness. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasCommand } from '../../domain/CanvasViewPort.js';

/** Dispatch the shared navigation command without depending on native pair capabilities. */
export const CanvasComparisonExit = defineComponent({
  name: 'CanvasComparisonExit',
  props: {
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Preserve a direct exit while resources fail or a host renegotiates. */
  setup(props) {
    return () =>
      h(
        'button',
        {
          type: 'button',
          class: 'sc-output-segment sc-output-comparison-exit',
          'data-sc-fixed-navigation': 'true',
          'aria-label': 'Exit comparison',
          title: 'Exit comparison',
          onClick: () => props.send({ type: 'comparison', enabled: false }),
        },
        'Exit',
      );
  },
});
