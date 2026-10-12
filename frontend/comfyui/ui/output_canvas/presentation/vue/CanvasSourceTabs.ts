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

import { defineComponent, h, onBeforeUnmount, onMounted, onUpdated, ref, type PropType } from 'vue';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';
import { CanvasSelectionPicker } from './CanvasSelectionPicker.js';
import { CanvasBatchPicker } from './CanvasBatchPicker.js';

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
    const strip = ref<HTMLElement | null>(null);
    const tabs = ref<HTMLElement | null>(null);
    const collapsed = ref(false);
    let observer: ResizeObserver | null = null;
    let observedTabs: HTMLElement | null = null;
    /** Measure settled content width while keeping the tab measurement independent of collapse. */
    function measure(): void {
      const parent = strip.value?.parentElement;
      if (!parent || !tabs.value || !parent.clientWidth) return;
      const fixed = [
        ...(strip.value?.querySelectorAll<HTMLElement>('[data-sc-fixed-navigation]') ?? []),
      ].reduce((width, element) => width + element.offsetWidth, 0);
      collapsed.value = tabs.value.scrollWidth + fixed + 24 > parent.clientWidth - 16;
    }
    onMounted(() => {
      if (typeof ResizeObserver === 'undefined' || !strip.value?.parentElement) return;
      observer = new ResizeObserver(measure);
      observer.observe(strip.value.parentElement);
      measure();
    });
    onUpdated(() => {
      if (observedTabs !== tabs.value) {
        if (observedTabs) observer?.unobserve(observedTabs);
        observedTabs = tabs.value;
        if (observedTabs) observer?.observe(observedTabs);
      }
      measure();
    });
    onBeforeUnmount(() => observer?.disconnect());
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
      return h('div', { class: 'sc-output-tabstrip', ref: strip }, [
        props.capabilities.commands.includes('batch')
          ? h('div', { 'data-sc-fixed-navigation': 'true' }, [
              h(CanvasBatchPicker, {
                count: Math.max(0, ...props.snapshot.sources.map((value) => value.batchCount)),
                grid: props.snapshot.view.mode === 'overview',
                includeGrid:
                  Boolean(source?.batchCount) && props.capabilities.commands.includes('overview'),
                selection,
                label: 'Output image',
                send: props.send,
              }),
            ])
          : null,
        props.snapshot.sources.length > 1
          ? h(
              'nav',
              {
                class: collapsed.value ? 'sc-output-tabs sc-output-tabs-measure' : 'sc-output-tabs',
                ref: tabs,
                'aria-hidden': collapsed.value ? 'true' : undefined,
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
                    tabIndex: collapsed.value
                      ? -1
                      : props.snapshot.view.selection?.sourceId === source.id ||
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
            )
          : null,
        collapsed.value
          ? h(CanvasSelectionPicker, {
              sources: props.snapshot.sources,
              selection,
              label: 'Output source',
              preview: true,
              includeBatch: false,
              send: props.send,
            })
          : null,
      ]);
    };
  },
});
