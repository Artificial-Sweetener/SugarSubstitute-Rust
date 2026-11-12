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
/** Render output sources as a segmented tab list whose selection slides between tabs. */

import { defineComponent, h, onBeforeUnmount, onMounted, onUpdated, ref, type PropType } from 'vue';
import type { CanvasSourceSummary } from '../../domain/CanvasViewPort.js';

/**
 * Keep source labels literal and move one selection indicator to the selected tab.
 *
 * The indicator is placed without motion when it first appears and slides on every later
 * selection or tab-size change. While `measuring`, the list stays laid out but hidden and out of
 * the tab order so its owner can decide whether it fits.
 */
export const CanvasSourceSlider = defineComponent({
  name: 'CanvasSourceSlider',
  props: {
    sources: { type: Array as PropType<readonly CanvasSourceSummary[]>, required: true },
    selectedId: { type: String as PropType<string | null>, default: null },
    measuring: { type: Boolean, default: false },
    choose: { type: Function as PropType<(index: number) => void>, required: true },
  },
  /** Bind keyboard navigation and indicator placement to the rendered tabs. */
  setup(props) {
    const list = ref<HTMLElement | null>(null);
    const indicator = ref<HTMLElement | null>(null);
    let observer: ResizeObserver | null = null;

    /** Cover the selected tab with the indicator; the stylesheet animates later moves. */
    function place(): void {
      const mark = indicator.value;
      const tab = list.value?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
      if (!mark) return;
      if (!tab) {
        mark.hidden = true;
        delete mark.dataset.placed;
        return;
      }
      mark.hidden = false;
      mark.style.transform = `translateX(${String(tab.offsetLeft)}px)`;
      mark.style.width = `${String(tab.offsetWidth)}px`;
      // Motion starts only after the first placement has been styled at its target.
      if (!mark.dataset.placed) {
        void mark.offsetWidth;
        mark.dataset.placed = 'true';
      }
    }
    onMounted(() => {
      place();
      if (typeof ResizeObserver === 'undefined' || !list.value) return;
      observer = new ResizeObserver(place);
      observer.observe(list.value);
    });
    onUpdated(place);
    onBeforeUnmount(() => observer?.disconnect());

    /** Follow tab-list keyboard conventions while keeping focus scoped to this overlay root. */
    function navigate(event: KeyboardEvent, index: number): void {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      const count = props.sources.length;
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
      list.value?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
      props.choose(next);
    }

    return () => {
      const known = props.sources.some((source) => source.id === props.selectedId);
      return h(
        'nav',
        {
          class: props.measuring ? 'sc-output-tabs sc-output-tabs-measure' : 'sc-output-tabs',
          ref: list,
          'aria-hidden': props.measuring ? 'true' : undefined,
          role: 'tablist',
          'aria-label': 'Workflow output sources',
          'data-capture-wheel': 'true',
        },
        [
          h('span', { class: 'sc-output-slider-indicator', ref: indicator, 'aria-hidden': 'true' }),
          ...props.sources.map((source, index) => {
            const selected = source.id === props.selectedId;
            return h(
              'button',
              {
                key: source.id,
                type: 'button',
                role: 'tab',
                'aria-selected': selected,
                tabIndex: props.measuring ? -1 : selected || (index === 0 && !known) ? 0 : -1,
                title: [...source.breadcrumb, source.label].join(' / '),
                onClick: () => props.choose(index),
                onKeydown: (event: KeyboardEvent) => navigate(event, index),
              },
              source.label,
            );
          }),
        ],
      );
    };
  },
});
