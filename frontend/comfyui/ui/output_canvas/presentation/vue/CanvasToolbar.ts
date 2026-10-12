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
/** Render inspection intents without deriving camera behavior from image elements. */

import { defineComponent, h, type PropType, type VNode } from 'vue';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';

/** Expose only operations supported by the attached surface. */
export const CanvasToolbar = defineComponent({
  name: 'CanvasToolbar',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    capabilities: { type: Object as PropType<CanvasCapabilities>, required: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Bind inspection intents without introducing a presentation-owned camera. */
  setup(props) {
    /** Render one capability-gated intent while preserving stable control identity. */
    function button(
      label: string,
      command: CanvasCommand,
      pressed?: boolean,
      disabled = false,
    ): VNode | null {
      if (!props.capabilities.commands.includes(command.type)) return null;
      return h(
        'button',
        {
          key: label,
          type: 'button',
          disabled,
          ...(pressed === undefined ? {} : { 'aria-pressed': pressed }),
          onClick: () => props.send(command),
        },
        label,
      );
    }
    return () => {
      const state = props.snapshot.view;
      const selection = state.selection;
      const selectedSource = props.snapshot.sources.find(
        (source) => source.id === selection?.sourceId,
      );
      const count = selectedSource?.batchCount ?? 0;
      const batch = selection?.batchIndex ?? 0;
      return h(
        'div',
        { class: 'sc-output-toolbar', role: 'toolbar', 'aria-label': 'Output inspection' },
        [
          button(
            'Auto',
            { type: 'follow', mode: state.follow === 'auto' ? 'manual' : 'auto' },
            state.follow === 'auto',
          ),
          button(
            'Overview',
            { type: 'overview', enabled: state.mode !== 'overview' },
            state.mode === 'overview',
          ),
          button('Link', { type: 'link', enabled: !state.linked }, state.linked),
          button('Fit', { type: 'fit' }),
          button('1:1', { type: 'native' }),
          button('Zoom out', { type: 'zoom', factor: 0.8 }),
          button('Zoom in', { type: 'zoom', factor: 1.25 }),
          button(
            'Previous image',
            { type: 'batch', index: Math.max(0, batch - 1) },
            undefined,
            !selection || batch === 0,
          ),
          h(
            'span',
            { class: 'sc-output-batch', 'aria-label': 'Batch position' },
            props.snapshot.preview
              ? 'Source preview'
              : selection
                ? `${batch + 1} / ${count}`
                : 'No image',
          ),
          button(
            'Next image',
            { type: 'batch', index: batch + 1 },
            undefined,
            !selection || batch + 1 >= count,
          ),
          button(
            'Compare',
            { type: 'comparison', enabled: !state.comparison },
            Boolean(state.comparison),
            !state.comparison &&
              (!selection ||
                !props.snapshot.selected ||
                props.snapshot.preview ||
                props.snapshot.selected.mediaKind !== 'image'),
          ),
        ],
      );
    };
  },
});
