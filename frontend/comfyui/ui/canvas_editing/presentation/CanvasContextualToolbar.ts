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
/** Float the contextual toolbar beside the selection, or along the canvas bottom without one. */
import {
  defineComponent,
  h,
  onMounted,
  onUpdated,
  ref,
  watch,
  type PropType,
  type VNode,
} from 'vue';
import type {
  CanvasEditController,
  CanvasEditSnapshot,
} from '../application/CanvasEditController.js';
import {
  CanvasToolbarPlacement,
  canvasEditSafeArea,
  type CanvasEditPoint,
  type CanvasEditRect,
} from '../domain/CanvasToolbarPlacement.js';
import { modifyPage, selectionPage, sessionPage, transformPage } from './CanvasEditPages.js';
import { CANVAS_EDIT_TEXT, CANVAS_TOOL_LABELS } from './CanvasEditText.js';

/** The canvas viewport in overlay-local CSS pixels and its display density. */
export interface CanvasEditViewport extends CanvasEditRect {
  readonly density: number;
}

/**
 * Render the page for the current surface and place it after layout.
 *
 * Selection bounds arrive in surface CSS pixels; offsetting them by the viewport origin maps them
 * into the overlay. The placement owner keeps its above/below choice while bounds follow pan and
 * zoom, and starts afresh when the page changes. The grip at the toolbar's start drags it; the
 * dropped place is kept until the page changes.
 */
export const CanvasContextualToolbar = defineComponent({
  name: 'CanvasContextualToolbar',
  props: {
    snapshot: { type: Object as PropType<CanvasEditSnapshot>, required: true },
    controller: { type: Object as PropType<CanvasEditController>, required: true },
    viewport: { type: Object as PropType<CanvasEditViewport>, required: true },
  },
  setup(props) {
    const element = ref<HTMLElement | null>(null);
    const placement = new CanvasToolbarPlacement();
    let shown: CanvasEditPoint | null = null;
    let drag: { readonly pointer: number; x: number; y: number } | null = null;
    watch(
      () => props.snapshot.surface.kind,
      () => placement.reset(),
    );
    /** Show the toolbar at one top-left point in overlay CSS pixels. */
    function show(toolbar: HTMLElement, point: CanvasEditPoint): void {
      shown = point;
      toolbar.style.transform = `translate(${point.x}px, ${point.y}px)`;
      toolbar.dataset.canvasPlacement = `${point.x},${point.y}`;
    }
    /** Begin a primary-button drag on the grip, capturing the pointer for its duration. */
    function grab(event: PointerEvent): void {
      if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
      event.preventDefault();
      event.stopPropagation();
      event.currentTarget.setPointerCapture(event.pointerId);
      drag = { pointer: event.pointerId, x: event.clientX, y: event.clientY };
    }
    /** Move by the pointer's travel since the last move. */
    function follow(event: PointerEvent): void {
      const toolbar = element.value;
      if (!drag || drag.pointer !== event.pointerId || !toolbar || !shown) return;
      event.preventDefault();
      const delta = { x: event.clientX - drag.x, y: event.clientY - drag.y };
      drag.x = event.clientX;
      drag.y = event.clientY;
      show(
        toolbar,
        placement.moveBy(
          shown,
          delta,
          { width: toolbar.offsetWidth, height: toolbar.offsetHeight },
          canvasEditSafeArea(props.viewport),
        ),
      );
    }
    /** End the drag; the dropped place stays with the placement owner. */
    function release(event: PointerEvent): void {
      if (drag?.pointer === event.pointerId) drag = null;
    }
    /** Position from the measured toolbar size; layout must precede placement. */
    function place(): void {
      const toolbar = element.value;
      if (!toolbar) return;
      const viewport = props.viewport;
      const bounds = props.snapshot.state.selection?.bounds;
      const context = bounds
        ? {
            left: viewport.left + bounds.left,
            top: viewport.top + bounds.top,
            width: bounds.width,
            height: bounds.height,
          }
        : null;
      show(
        toolbar,
        placement.position(
          context,
          { width: toolbar.offsetWidth, height: toolbar.offsetHeight },
          canvasEditSafeArea(viewport),
          viewport.density,
        ),
      );
    }
    onMounted(place);
    onUpdated(place);
    return () => {
      const children = toolbarPage(props.snapshot, props.controller);
      if (!children) return null;
      return h(
        'div',
        {
          ref: element,
          class: 'sc-canvas-edit-chrome sc-canvas-edit-row sc-canvas-edit-toolbar',
          role: 'toolbar',
          'aria-label': toolbarLabel(props.snapshot),
          'data-canvas-surface': props.snapshot.surface.kind,
        },
        [
          h('div', {
            class: 'sc-canvas-edit-toolbar-grip',
            title: CANVAS_EDIT_TEXT.moveToolbar,
            'aria-label': CANVAS_EDIT_TEXT.moveToolbar,
            'data-canvas-toolbar-grip': '',
            onPointerdown: grab,
            onPointermove: follow,
            onPointerup: release,
            onPointercancel: release,
            onLostpointercapture: release,
          }),
          ...children,
        ],
      );
    };
  },
});

function toolbarLabel(snapshot: CanvasEditSnapshot): string {
  const surface = snapshot.surface;
  if (surface.kind === 'modify') return CANVAS_EDIT_TEXT.modifySelection;
  if (surface.kind === 'transform') return CANVAS_EDIT_TEXT.transform;
  if (surface.kind === 'session') return CANVAS_TOOL_LABELS[surface.tool];
  return CANVAS_EDIT_TEXT.selectionActions;
}

function toolbarPage(snapshot: CanvasEditSnapshot, edit: CanvasEditController): VNode[] | null {
  const surface = snapshot.surface;
  switch (surface.kind) {
    case 'selection':
      return selectionPage(snapshot, edit);
    case 'modify':
      return modifyPage(snapshot, edit);
    case 'transform':
      return transformPage(snapshot, edit, surface.target);
    case 'session':
      return sessionPage(snapshot, edit, surface.tool);
    default:
      return null;
  }
}
