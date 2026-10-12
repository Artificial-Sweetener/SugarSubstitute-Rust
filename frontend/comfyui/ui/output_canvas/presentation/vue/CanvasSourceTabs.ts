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
/** Render source navigation from renderer-independent view snapshots. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasCommand, CanvasViewSnapshot } from '../../domain/CanvasViewPort.js';

/** Keep source labels literal and retain the chosen batch when switching. */
export const CanvasSourceTabs = defineComponent({
  name: 'CanvasSourceTabs',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Bind source navigation while preserving the selected batch position. */
  setup(props) {
    return () =>
      h(
        'nav',
        { class: 'sc-output-tabs', 'aria-label': 'Workflow output sources' },
        props.snapshot.sources.map((source) =>
          h(
            'button',
            {
              key: source.id,
              type: 'button',
              'aria-pressed': props.snapshot.view.selection?.sourceId === source.id,
              title: [...source.breadcrumb, source.label].join(' / '),
              onClick: () =>
                props.send({
                  type: 'select',
                  selection: {
                    sourceId: source.id,
                    batchIndex: props.snapshot.view.selection?.batchIndex ?? 0,
                  },
                }),
            },
            source.label,
          ),
        ),
      );
  },
});
