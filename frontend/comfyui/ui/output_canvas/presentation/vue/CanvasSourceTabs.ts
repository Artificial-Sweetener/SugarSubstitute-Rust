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

import { defineComponent, h, type PropType, type VNode } from 'vue';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';

/** Keep source labels literal and retain the chosen batch when switching. */
export const CanvasSourceTabs = defineComponent({
  name: 'CanvasSourceTabs',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    capabilities: { type: Object as PropType<CanvasCapabilities>, required: true },
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
    return () => {
      const selection = props.snapshot.view.selection;
      const source = props.snapshot.sources.find((value) => value.id === selection?.sourceId);
      const count = source?.batchCount ?? 0;
      const batch = selection?.batchIndex ?? 0;
      const navigation: VNode[] = [];
      if (count > 1 && selection && props.capabilities.commands.includes('batch'))
        navigation.push(
          h(
            'button',
            {
              type: 'button',
              class: 'sc-output-batch-arrow',
              'aria-label': 'Previous image',
              disabled: batch === 0,
              onClick: () => props.send({ type: 'batch', index: Math.max(0, batch - 1) }),
            },
            '\u2039',
          ),
          h(
            'span',
            { class: 'sc-output-batch', 'aria-label': 'Batch position' },
            props.snapshot.preview
              ? 'Preview'
              : props.snapshot.view.comparison && batch >= count
                ? `Image ${batch + 1}`
                : `${batch + 1} / ${count}`,
          ),
          h(
            'button',
            {
              type: 'button',
              class: 'sc-output-batch-arrow',
              'aria-label': 'Next image',
              disabled: batch + 1 >= count,
              onClick: () => props.send({ type: 'batch', index: batch + 1 }),
            },
            '\u203a',
          ),
        );
      return h('div', { class: 'sc-output-tabstrip' }, [
        h('div', { class: 'sc-output-batch-navigation' }, navigation),
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
        ),
      ]);
    };
  },
});
