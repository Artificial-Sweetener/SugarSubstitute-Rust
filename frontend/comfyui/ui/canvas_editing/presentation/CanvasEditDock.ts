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
/** Dock the history bar, tool options and tool rail beside the host's canvas chrome. */
import { defineComponent, h, onBeforeUnmount, onMounted, onUpdated, ref, type PropType } from 'vue';
import type {
  CanvasEditController,
  CanvasEditSnapshot,
} from '../application/CanvasEditController.js';
import { canvasEditDockPlacement } from '../domain/CanvasEditDockLayout.js';
import { canvasEditToolAvailable } from '../domain/CanvasEditTools.js';
import {
  CANVAS_EDIT_CHROME_INSET,
  type CanvasEditPoint,
  type CanvasEditRect,
} from '../domain/CanvasToolbarPlacement.js';
import { CanvasBrushOptions } from './CanvasBrushOptions.js';
import { iconButton } from './CanvasEditPages.js';
import { CANVAS_EDIT_TEXT } from './CanvasEditText.js';
import { CanvasToolRail } from './CanvasToolRail.js';

/**
 * Render the top bar (document history, then the active tool's options) and the vertical rail,
 * then place both around the host's chrome after layout.
 *
 * Placement follows every render, every size change of either surface (the options expand in
 * place), and `layoutRevision`, which the owner advances when the canvas allocation changes.
 */
export const CanvasEditDock = defineComponent({
  name: 'CanvasEditDock',
  props: {
    snapshot: { type: Object as PropType<CanvasEditSnapshot>, required: true },
    controller: { type: Object as PropType<CanvasEditController>, required: true },
    maskColor: { type: String as PropType<string | null>, default: null },
    /** Host chrome to keep clear, in the dock's CSS-pixel space. */
    obstacles: {
      type: Function as PropType<() => readonly CanvasEditRect[]>,
      default: () => [],
    },
    layoutRevision: { type: Number, default: 0 },
  },
  setup(props) {
    const topBar = ref<HTMLElement | null>(null);
    const rail = ref<HTMLElement | null>(null);
    let observer: ResizeObserver | null = null;
    /** Position both surfaces from their measured sizes; layout must precede placement. */
    function place(): void {
      const bar = topBar.value,
        column = rail.value;
      if (!bar || !column) return;
      const placement = canvasEditDockPlacement(
        props.obstacles(),
        { width: bar.offsetWidth, height: bar.offsetHeight },
        { width: column.offsetWidth, height: column.offsetHeight },
      );
      move(bar, placement.topBar);
      move(column, placement.rail);
      column.style.maxHeight = `calc(100% - ${placement.rail.y + CANVAS_EDIT_CHROME_INSET}px)`;
    }
    onMounted(() => {
      place();
      if (typeof ResizeObserver === 'undefined') return;
      observer = new ResizeObserver(() => place());
      if (topBar.value) observer.observe(topBar.value);
      if (rail.value) observer.observe(rail.value);
    });
    onUpdated(place);
    onBeforeUnmount(() => observer?.disconnect());
    return () => {
      void props.layoutRevision;
      const edit = props.controller;
      const state = props.snapshot.state;
      const tool = state.tool === 'brush' || state.tool === 'eraser' ? state.tool : null;
      return h('div', { class: 'sc-canvas-edit-dock' }, [
        h('div', { ref: topBar, class: 'sc-canvas-edit-topbar' }, [
          h(
            'div',
            {
              class: 'sc-canvas-edit-chrome sc-canvas-edit-row',
              role: 'group',
              'aria-label': CANVAS_EDIT_TEXT.history,
            },
            [
              iconButton(
                'undo',
                CANVAS_EDIT_TEXT.undo,
                'undo',
                () => edit.execute({ type: 'history', action: 'undo' }),
                state.history.undo,
              ),
              iconButton(
                'redo',
                CANVAS_EDIT_TEXT.redo,
                'redo',
                () => edit.execute({ type: 'history', action: 'redo' }),
                state.history.redo,
              ),
            ],
          ),
          tool && canvasEditToolAvailable(state, tool)
            ? h(CanvasBrushOptions, {
                key: 'options',
                tool,
                options: state.options[tool],
                color: props.maskColor,
                change: (target: 'brush' | 'eraser', options: typeof state.options.brush) =>
                  edit.setBrushOptions(target, options),
              })
            : null,
        ]),
        h('div', { ref: rail, class: 'sc-canvas-edit-rail-dock' }, [
          h(CanvasToolRail, {
            slots: props.snapshot.slots,
            selectSlot: (slot: Parameters<CanvasEditController['selectSlot']>[0]) =>
              edit.selectSlot(slot),
            selectTool: (selected: Parameters<CanvasEditController['selectTool']>[0]) =>
              edit.selectTool(selected),
          }),
        ]),
      ]);
    };
  },
});

/** Translate one surface to its placed point and record it for inspection. */
function move(element: HTMLElement, point: CanvasEditPoint): void {
  element.style.transform = `translate(${point.x}px, ${point.y}px)`;
  element.dataset.canvasPlacement = `${point.x},${point.y}`;
}
