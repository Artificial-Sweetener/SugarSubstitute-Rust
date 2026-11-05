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
/** Render a combo-box control whose themed choice rows align the selected row over it. */

import { defineComponent, h, nextTick, onBeforeUnmount, ref, Teleport, type PropType } from 'vue';
import { FloatingSurface } from '../../../menus/FloatingSurface.js';

export interface CanvasPickerChoice {
  readonly value: string;
  readonly label: string;
  /** Shown but not choosable, such as a scene with nothing to show yet. */
  readonly disabled?: boolean;
}

/** Adapt choice focus and selected-row alignment to the shared floating transport. */
export const CanvasAnchoredPicker = defineComponent({
  name: 'CanvasAnchoredPicker',
  props: {
    label: { type: String, required: true },
    value: { type: String, required: true },
    text: { type: String, default: '' },
    items: { type: Array as PropType<readonly CanvasPickerChoice[]>, required: true },
    centered: { type: Boolean, default: false },
    /** Size the control to its longest choice so changing the choice never moves its neighbours. */
    stable: { type: Boolean, default: false },
    choose: { type: Function as PropType<(value: string) => void>, required: true },
  },
  /** Keep popup work finite and scoped to the control's owning document. */
  setup(props) {
    const anchor = ref<HTMLButtonElement | null>(null);
    const popup = ref<HTMLElement | null>(null);
    const opened = ref(false);
    const active = ref(0);
    const geometry = ref({ left: 0, top: 0, width: 0, height: 0 });
    let surface: FloatingSurface | null = null;

    /** Retire the shared surface and optionally return focus after deliberate dismissal. */
    function close(restoreFocus = false): void {
      surface?.close();
      surface = null;
      opened.value = false;
      if (restoreFocus) anchor.value?.focus();
    }

    /** Focus a concrete row and let the bounded scroll viewport reveal it. */
    function focusRow(): void {
      const row = popup.value?.querySelectorAll<HTMLButtonElement>('[role="option"]')[active.value];
      row?.focus({ preventScroll: true });
      row?.scrollIntoView?.({ block: 'nearest' });
    }

    /** Align the selected row with the anchor while bounding overflow to the viewport. */
    async function open(): Promise<void> {
      if (opened.value) {
        close(true);
        return;
      }
      const button = anchor.value;
      const viewport = button?.ownerDocument.defaultView;
      if (!button || !viewport || !props.items.length) return;
      const rect = button.getBoundingClientRect();
      const rowHeight = Math.max(28, rect.height);
      const stride = rowHeight + 2;
      active.value = Math.max(
        0,
        props.items.findIndex((item) => item.value === props.value),
      );
      const above = Math.max(0, Math.floor((rect.top - 8) / stride));
      const below = Math.max(0, Math.floor((viewport.innerHeight - rect.bottom - 8) / stride));
      const visible = Math.min(props.items.length, above + 1 + below);
      const slot = Math.min(active.value, above, visible - 1);
      const width = Math.min(Math.max(rect.width, 34) + 16, viewport.innerWidth - 16);
      const height = visible * stride - 2 + 16;
      geometry.value = {
        left: Math.max(8, Math.min(rect.left - 8, viewport.innerWidth - width - 8)),
        top: Math.max(0, Math.min(rect.top - 8 - slot * stride, viewport.innerHeight - height)),
        width,
        height,
      };
      opened.value = true;
      await nextTick();
      if (!opened.value || !popup.value) return;
      // Rows are at least the control's width and grow to fit their longest label.
      const fitted = popup.value.offsetWidth;
      if (fitted > width)
        geometry.value = {
          ...geometry.value,
          left: Math.max(8, Math.min(geometry.value.left, viewport.innerWidth - fitted - 8)),
        };
      surface = new FloatingSurface(popup.value);
      surface.open({
        anchor: { kind: 'pointer', x: geometry.value.left, y: geometry.value.top },
        maxHeight: geometry.value.height,
        dismiss: () => close(),
        containsTarget: (target) => button.contains(target),
      });
      popup.value.scrollTop = Math.max(0, (active.value - slot) * stride);
      focusRow();
    }

    /** Emit only a choice still present in the current snapshot, then restore control focus. */
    function select(index: number): void {
      const item = props.items[index];
      if (!item || item.disabled) return;
      close(true);
      props.choose(item.value);
    }

    /** Navigate local choice rows without forwarding canvas or graph shortcuts. */
    function keydown(event: KeyboardEvent): void {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        event.stopPropagation();
        select(active.value);
        return;
      }
      if (event.key === 'Escape' || event.key === 'Tab') {
        if (event.key === 'Escape') event.preventDefault();
        event.stopPropagation();
        close(true);
        return;
      }
      const index =
        event.key === 'ArrowDown'
          ? Math.min(props.items.length - 1, active.value + 1)
          : event.key === 'ArrowUp'
            ? Math.max(0, active.value - 1)
            : event.key === 'Home'
              ? 0
              : event.key === 'End'
                ? props.items.length - 1
                : null;
      if (index === null) return;
      event.preventDefault();
      event.stopPropagation();
      active.value = index;
      focusRow();
    }

    onBeforeUnmount(() => close());
    return () =>
      h(
        'span',
        {
          class: 'sc-output-choice',
          'data-centered': props.centered ? 'true' : undefined,
          'data-stable': props.stable ? 'true' : undefined,
          style: props.stable
            ? {
                '--sc-choice-label-chars': String(
                  Math.max(1, ...props.items.map((item) => item.label.length)),
                ),
              }
            : undefined,
        },
        [
          h(
            'button',
            {
              ref: anchor,
              type: 'button',
              class: 'sc-output-segment',
              'aria-label': props.label,
              'aria-haspopup': 'listbox',
              'aria-expanded': opened.value,
              disabled: !props.items.length,
              onClick: open,
            },
            [
              h(
                'span',
                { class: 'sc-output-choice-label' },
                props.text ||
                  props.items.find((item) => item.value === props.value)?.label ||
                  'Output',
              ),
              h('span', { class: 'sc-output-chevron', 'aria-hidden': 'true' }),
            ],
          ),
          opened.value
            ? h(Teleport, { to: anchor.value?.ownerDocument.body }, [
                h(
                  'div',
                  {
                    ref: popup,
                    class: 'sc-output-picker-popup',
                    role: 'listbox',
                    'aria-label': `${props.label} choices`,
                    'data-centered': props.centered ? 'true' : undefined,
                    style: {
                      left: `${geometry.value.left}px`,
                      top: `${geometry.value.top}px`,
                      minWidth: `${geometry.value.width}px`,
                      maxWidth: `${String(Math.max(geometry.value.width, (anchor.value?.ownerDocument.defaultView?.innerWidth ?? 0) - 16))}px`,
                      maxHeight: `${geometry.value.height}px`,
                    },
                    onKeydown: keydown,
                  },
                  props.items.map((item, index) =>
                    h(
                      'button',
                      {
                        key: item.value,
                        type: 'button',
                        role: 'option',
                        'aria-selected': index === active.value,
                        'aria-disabled': item.disabled ? 'true' : undefined,
                        tabIndex: index === active.value ? 0 : -1,
                        onFocus: () => {
                          active.value = index;
                        },
                        onClick: () => select(index),
                      },
                      item.label,
                    ),
                  ),
                ),
              ])
            : null,
        ],
      );
  },
});
