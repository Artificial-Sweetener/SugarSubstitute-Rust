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
/** Adapt GimpSpinScale's entry progress and relative gestures to the existing native numeric binding. */
// Behavior adapted from GIMP libgimpwidgets/gimpspinscale.c, Copyright 2010 Michael Natterer,
// 2012 Øyvind Kolås, GPL-3.0-or-later. See third_party/manifest.toml.
import { computed, defineComponent, h, useHost } from 'vue';
import { publishGimpInput } from './GimpInputEvents.js';

interface RelativeGesture {
  pointerId: number;
  x: number;
  value: number;
  width: number;
}

/** Draw one GTK entry with a progress fill and an editable native number supplied by the host. */
export const GimpSpinScaleComponent = defineComponent({
  name: 'GimpSpinScale',
  inheritAttrs: false,
  props: {
    min: { default: 0 },
    max: { default: 100 },
    step: { default: 1 },
    value: { default: 0 },
    disabled: Boolean,
  },
  /** Keep only pointer-gesture state here; publish edits through the shared native value path. */
  setup(props, { attrs }) {
    const host = useHost();
    let relative: RelativeGesture | null = null;
    const fraction = computed(() => {
      const extent = Number(props.max) - Number(props.min);
      return extent > 0
        ? Math.max(0, Math.min(1, (Number(props.value) - Number(props.min)) / extent))
        : 0;
    });
    /** Let native absolute range gestures run, reserving Shift/right drag for GIMP's fine adjustment. */
    function begin(event: PointerEvent): void {
      const input = event.currentTarget;
      if (
        !(input instanceof HTMLInputElement) ||
        input.disabled ||
        (event.button !== 2 && !event.shiftKey)
      )
        return;
      if (event.button !== 0 && event.button !== 2) return;
      event.preventDefault();
      event.stopPropagation();
      input.focus();
      relative = {
        pointerId: event.pointerId,
        x: event.clientX,
        value: input.valueAsNumber,
        width: input.getBoundingClientRect().width,
      };
      input.setPointerCapture(event.pointerId);
    }
    /** Apply GIMP's one-tenth relative speed, then let the native range clamp and quantize its value. */
    function move(event: PointerEvent): void {
      const input = event.currentTarget;
      if (
        !relative ||
        relative.pointerId !== event.pointerId ||
        !(input instanceof HTMLInputElement)
      )
        return;
      const extent = Number(props.max) - Number(props.min);
      const direction = getComputedStyle(input).direction === 'rtl' ? -1 : 1;
      let value =
        relative.value +
        ((direction * (event.clientX - relative.x)) / Math.max(1, relative.width)) * extent * 0.1;
      if (event.ctrlKey) {
        const page = Number(props.step) * 10;
        if (page > 0) value = Math.round(value / page) * page;
      }
      input.value = String(value);
      input.dispatchEvent(new Event('input', { bubbles: true }));
      event.preventDefault();
      event.stopPropagation();
    }
    /** Finish one fine-adjustment gesture without generating a second canonical edit. */
    function finish(event: PointerEvent): void {
      const input = event.currentTarget;
      if (
        !relative ||
        relative.pointerId !== event.pointerId ||
        !(input instanceof HTMLInputElement)
      )
        return;
      relative = null;
      if (input.hasPointerCapture(event.pointerId)) input.releasePointerCapture(event.pointerId);
      input.dispatchEvent(new Event('change', { bubbles: true }));
    }
    return () =>
      h(
        'div',
        { class: 'spin-scale', style: { '--sc-gimp-scale-fraction': String(fraction.value) } },
        [
          h('span', { class: 'spin-scale-progress', 'aria-hidden': 'true' }),
          h(
            'span',
            { class: 'spin-scale-label', 'aria-hidden': 'true' },
            String(attrs.dataScNumberLabel ?? 'Value'),
          ),
          h('input', {
            type: 'range',
            ...props,
            tabindex: attrs.dataScNumberLabel === undefined ? 0 : -1,
            'aria-label': attrs.ariaLabel ?? 'Value',
            onInput: (event: Event) => publishGimpInput(host, event, 'value'),
            onChange: (event: Event) => publishGimpInput(host, event, 'value'),
            onPointerdown: begin,
            onPointermove: move,
            onPointerup: finish,
            onPointercancel: finish,
            onLostpointercapture: () => {
              relative = null;
            },
            onContextmenu: (event: Event) => event.preventDefault(),
          }),
        ],
      );
  },
});
