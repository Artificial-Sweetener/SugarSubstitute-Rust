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
/** Render the one canvas navigation bar from renderer-independent view snapshots. */

import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref,
  type ComponentPublicInstance,
  type PropType,
} from 'vue';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';
import type { CanvasRouteId, CanvasRouteState } from '../../domain/CanvasRoute.js';
import { CanvasSelectionPicker } from './CanvasSelectionPicker.js';
import { CanvasBatchPicker } from './CanvasBatchPicker.js';
import { CanvasRoutePicker } from './CanvasRoutePicker.js';
import { CanvasSourceSlider } from './CanvasSourceSlider.js';
import { CanvasScenePicker } from './CanvasScenePicker.js';

/**
 * Show only the navigation relevant to the current canvas, in one bar.
 *
 * The canvas, scene and batch choices are combo boxes. The final layer, the output sources of the
 * batch being viewed, is a sliding tab list that collapses to a picker when the bar lacks the
 * width for it. Input shows only the canvas choice, and the bar is absent when nothing applies.
 */
export const CanvasNavigationBar = defineComponent({
  name: 'CanvasNavigationBar',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    capabilities: { type: Object as PropType<CanvasCapabilities>, required: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
    route: { type: Object as PropType<CanvasRouteState | null>, default: null },
    chooseRoute: { type: Function as PropType<(route: CanvasRouteId) => void>, required: true },
  },
  /** Bind source navigation while preserving the selected batch position. */
  setup(props) {
    const strip = ref<HTMLElement | null>(null);
    const slider = ref<ComponentPublicInstance | null>(null);
    const collapsed = ref(false);
    let observer: ResizeObserver | null = null;
    let observedParent: Element | null = null;
    let observedTabs: Element | null = null;

    /** Read the rendered tab list, which stays laid out while collapsed for measurement. */
    function tabs(): HTMLElement | null {
      const element: unknown = slider.value?.$el;
      return element instanceof HTMLElement ? element : null;
    }
    /** Measure settled content width while keeping the tab measurement independent of collapse. */
    function measure(): void {
      const parent = strip.value?.parentElement;
      const list = tabs();
      if (!list) collapsed.value = false;
      if (!parent || !list || !parent.clientWidth) return;
      const fixed = [
        ...(strip.value?.querySelectorAll<HTMLElement>('[data-sc-fixed-navigation]') ?? []),
      ].reduce((width, element) => width + element.offsetWidth, 0);
      collapsed.value = list.scrollWidth + fixed + 24 > parent.clientWidth - 16;
    }
    /** Observe the bar's container and tab list as they appear, change, or disappear. */
    function observe(): void {
      if (typeof ResizeObserver === 'undefined') return;
      observer ??= new ResizeObserver(measure);
      const parent = strip.value?.parentElement ?? null;
      if (parent !== observedParent) {
        if (observedParent) observer.unobserve(observedParent);
        observedParent = parent;
        if (parent) observer.observe(parent);
      }
      const list = tabs();
      if (list !== observedTabs) {
        if (observedTabs) observer.unobserve(observedTabs);
        observedTabs = list;
        if (list) observer.observe(list);
      }
    }
    onMounted(() => {
      observe();
      measure();
    });
    onUpdated(() => {
      observe();
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

    return () => {
      const { snapshot, capabilities, route } = props;
      const outputs =
        route?.route !== 'input' &&
        capabilities.commands.includes('select-source') &&
        (snapshot.sources.length > 1 || (snapshot.sources[0]?.batchCount ?? 0) > 1);
      const scenes = route?.route !== 'input' && snapshot.scenes.length > 1;
      if (!route?.inputAvailable && !outputs && !scenes) return null;
      const selection = snapshot.view.selection;
      const source = snapshot.sources.find((value) => value.id === selection?.sourceId);
      return h('div', { class: 'sc-output-tabstrip sc-output-navigation', ref: strip }, [
        route ? h(CanvasRoutePicker, { state: route, choose: props.chooseRoute }) : null,
        scenes && capabilities.commands.includes('select-scene')
          ? h(CanvasScenePicker, {
              scenes: snapshot.scenes,
              scene: snapshot.view.scene,
              send: props.send,
            })
          : null,
        outputs && capabilities.commands.includes('batch')
          ? h('div', { 'data-sc-fixed-navigation': 'true' }, [
              h(CanvasBatchPicker, {
                count: Math.max(0, ...snapshot.sources.map((value) => value.batchCount)),
                grid: snapshot.view.mode === 'overview',
                includeGrid:
                  Boolean(source?.batchCount) && capabilities.commands.includes('overview'),
                selection,
                label: 'Output image',
                send: props.send,
              }),
            ])
          : null,
        outputs && snapshot.sources.length > 1
          ? h(CanvasSourceSlider, {
              ref: slider,
              sources: snapshot.sources,
              selectedId: selection?.sourceId ?? null,
              measuring: collapsed.value,
              choose,
            })
          : null,
        outputs && collapsed.value
          ? h(CanvasSelectionPicker, {
              sources: snapshot.sources,
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
