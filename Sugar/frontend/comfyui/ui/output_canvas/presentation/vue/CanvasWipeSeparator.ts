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
/** Publish normalized wipe intents from a reusable, keyboard-operable overlay separator. */

import { defineComponent, h, onBeforeUnmount, shallowRef, watch, type PropType } from 'vue';
import type { CanvasCommand, CanvasViewSnapshot } from '../../domain/CanvasViewPort.js';

/** Keep divider interaction local while the attached rendering surface owns image clipping. */
export const CanvasWipeSeparator = defineComponent({
  name: 'CanvasWipeSeparator',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Capture one divider pointer and coalesce normalized commands without image or camera access. */
  setup(props) {
    const local = shallowRef<number | null>(null);
    let pointer: { readonly id: number; readonly element: HTMLElement } | null = null;
    let pending: number | null = null;
    let cancel: (() => void) | null = null;
    /** Send the latest drag position once, using current orientation. */
    function flush(): void {
      cancel?.();
      cancel = null;
      const split = pending;
      pending = null;
      const comparison = props.snapshot.view.comparison;
      if (split !== null && comparison)
        props.send({ type: 'wipe', orientation: comparison.orientation, split });
    }
    /** Release capture before root teardown or comparison replacement. */
    function release(completed: boolean): void {
      if (completed) flush();
      else {
        cancel?.();
        cancel = null;
        pending = null;
        local.value = null;
      }
      const captured = pointer;
      pointer = null;
      if (captured?.element.hasPointerCapture(captured.id))
        captured.element.releasePointerCapture(captured.id);
    }
    /** Normalize pointer travel against this root's overlay rectangle in CSS coordinates. */
    function move(event: PointerEvent): void {
      if (!pointer || pointer.id !== event.pointerId) return;
      const comparison = props.snapshot.view.comparison;
      const bounds = pointer.element.parentElement?.getBoundingClientRect();
      if (!comparison || !bounds) return;
      const vertical = comparison.orientation === 'vertical';
      const size = vertical ? bounds.width : bounds.height;
      if (size <= 0) return;
      event.preventDefault();
      event.stopPropagation();
      const position = vertical ? event.clientX - bounds.left : event.clientY - bounds.top;
      const split = Math.max(0, Math.min(1, position / size));
      local.value = split;
      pending = split;
      const window = pointer.element.ownerDocument.defaultView;
      if (!cancel && window) {
        const frame = window.requestAnimationFrame(() => {
          cancel = null;
          flush();
        });
        cancel = () => window.cancelAnimationFrame(frame);
      }
    }
    /** Apply separator keyboard conventions without stealing modified host shortcuts. */
    function key(event: KeyboardEvent): void {
      const comparison = props.snapshot.view.comparison;
      if (!comparison || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
      const negative = comparison.orientation === 'vertical' ? 'ArrowLeft' : 'ArrowUp';
      const positive = comparison.orientation === 'vertical' ? 'ArrowRight' : 'ArrowDown';
      const split =
        event.key === 'Home'
          ? 0
          : event.key === 'End'
            ? 1
            : event.key === negative
              ? Math.max(0, (local.value ?? comparison.split) - 0.05)
              : event.key === positive
                ? Math.min(1, (local.value ?? comparison.split) + 0.05)
                : null;
      if (split === null) return;
      event.preventDefault();
      event.stopPropagation();
      local.value = split;
      pending = split;
      flush();
    }
    watch(
      () => props.snapshot.revision,
      () => {
        if (!pointer) local.value = null;
      },
    );
    watch(
      () => props.snapshot.view.comparison?.orientation,
      () => release(false),
    );
    watch(
      () => JSON.stringify(props.snapshot.address),
      () => release(false),
    );
    onBeforeUnmount(() => release(false));
    return () => {
      const comparison = props.snapshot.view.comparison;
      if (!comparison) return null;
      const vertical = comparison.orientation === 'vertical';
      const split = local.value ?? comparison.split;
      return h(
        'div',
        {
          class: 'sc-output-wipe',
          role: 'separator',
          tabindex: 0,
          'aria-label': 'A/B comparison divider',
          'aria-orientation': comparison.orientation,
          'aria-valuemin': 0,
          'aria-valuemax': 100,
          'aria-valuenow': Math.round(split * 100),
          'data-orientation': comparison.orientation,
          style: vertical ? { left: `${split * 100}%` } : { top: `${split * 100}%` },
          onPointerdown: (event: PointerEvent) => {
            const element = event.currentTarget;
            if (
              !(element instanceof HTMLElement) ||
              pointer ||
              event.button !== 0 ||
              event.ctrlKey ||
              event.metaKey ||
              event.altKey ||
              event.shiftKey
            )
              return;
            event.preventDefault();
            event.stopPropagation();
            pointer = { id: event.pointerId, element };
            element.focus({ preventScroll: true });
            element.setPointerCapture(event.pointerId);
          },
          onPointermove: move,
          onPointerup: (event: PointerEvent) => {
            if (event.pointerId === pointer?.id) {
              event.stopPropagation();
              release(true);
            }
          },
          onPointercancel: (event: PointerEvent) => {
            if (event.pointerId === pointer?.id) release(false);
          },
          onLostpointercapture: (event: PointerEvent) => {
            if (event.pointerId === pointer?.id) release(false);
          },
          onKeydown: key,
        },
        h('span', { 'aria-hidden': 'true' }, vertical ? '↔' : '↕'),
      );
    };
  },
});
