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
    /** Activate a declared source without treating its live preview as a numbered batch. */
    function choose(index: number): void {
      const source = props.snapshot.sources[index];
      if (source)
        props.send({
          type: 'select-source',
          selection: {
            sourceId: source.id,
            batchIndex: props.snapshot.view.selection?.batchIndex ?? 0,
          },
        });
    }
    /** Follow tab-list keyboard conventions while keeping focus scoped to this overlay root. */
    function navigate(event: KeyboardEvent, index: number): void {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      const count = props.snapshot.sources.length;
      const next =
        event.key === 'Home'
          ? 0
          : event.key === 'End'
            ? count - 1
            : event.key === 'ArrowRight'
              ? (index + 1) % count
              : event.key === 'ArrowLeft'
                ? (index + count - 1) % count
                : null;
      if (next === null || !count) return;
      event.preventDefault();
      event.stopPropagation();
      const button = event.currentTarget;
      if (button instanceof HTMLButtonElement)
        button.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
      choose(next);
    }
    return () =>
      h(
        'nav',
        {
          class: 'sc-output-tabs',
          role: 'tablist',
          'aria-label': 'Workflow output sources',
          'data-capture-wheel': 'true',
        },
        props.snapshot.sources.map((source, index) =>
          h(
            'button',
            {
              key: source.id,
              type: 'button',
              role: 'tab',
              'aria-selected': props.snapshot.view.selection?.sourceId === source.id,
              tabIndex:
                props.snapshot.view.selection?.sourceId === source.id ||
                (index === 0 &&
                  !props.snapshot.sources.some(
                    (value) => value.id === props.snapshot.view.selection?.sourceId,
                  ))
                  ? 0
                  : -1,
              title: [...source.breadcrumb, source.label].join(' / '),
              onClick: () => choose(index),
              onKeydown: (event: KeyboardEvent) => navigate(event, index),
            },
            source.label,
          ),
        ),
      );
  },
});
