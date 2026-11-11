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
/** Render the desktop stack's compact or two-line row using safe literal text and stable controls. */
import { defineComponent, h, type PropType } from 'vue';
import type { CubeStackRowModel } from '../CubeStackView.js';
import { CubeStackIcon } from './CubeStackIcon.js';

/** Share row geometry and visual states between live stack navigation and the cart. */
export const CubeStackRow = defineComponent({
  name: 'SugarCubesStackRow',
  inheritAttrs: false,
  props: {
    row: { type: Object as PropType<CubeStackRowModel>, required: true },
    compact: { type: Boolean, default: false },
    pending: { type: Boolean, default: false },
    removable: { type: Boolean, default: false },
    activate: { type: Function as PropType<() => void>, required: true },
    remove: { type: Function as PropType<() => void>, required: true },
    contextMenu: { type: Function as PropType<(event: MouseEvent) => void>, required: true },
  },
  setup(props, { attrs }) {
    return () =>
      h(
        'li',
        {
          ...attrs,
          class: ['sc-cube-stack-row', attrs.class],
          'data-selected': props.row.selected || undefined,
          'data-bypassed': props.row.bypassed || undefined,
          'data-severity': props.row.severity,
          'data-row-id': props.row.id,
        },
        [
          h(
            'button',
            {
              class: 'sc-cube-stack-row__activate',
              type: 'button',
              title: props.row.tooltip,
              'aria-label': props.row.label,
              'aria-description': props.row.targetModel
                ? `Model: ${props.row.targetModel}`
                : undefined,
              'aria-current': props.row.selected ? 'true' : undefined,
              'aria-disabled': props.pending || undefined,
              onKeydown: (event: KeyboardEvent) => {
                if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return;
                event.preventDefault();
                const target = event.currentTarget;
                if (props.pending || !(target instanceof HTMLElement)) return;
                const bounds = target.getBoundingClientRect();
                props.contextMenu(
                  new MouseEvent('contextmenu', { clientX: bounds.left, clientY: bounds.bottom }),
                );
              },
              onClick: () => {
                if (!props.pending) props.activate();
              },
              onContextmenu: (event: MouseEvent) => {
                event.preventDefault();
                if (!props.pending) props.contextMenu(event);
              },
            },
            [
              h(CubeStackIcon, { icon: props.row.icon, targetModel: props.row.targetModel }),
              h(
                'span',
                { class: 'sc-cube-stack-row__text', 'aria-hidden': props.compact || undefined },
                [
                  h('span', { class: 'sc-cube-stack-row__label' }, props.row.label),
                  h('span', { class: 'sc-cube-stack-row__secondary' }, props.row.secondary),
                ],
              ),
            ],
          ),
          props.removable
            ? h(
                'button',
                {
                  class: 'sc-cube-stack-row__remove',
                  type: 'button',
                  title: `Remove ${props.row.label}`,
                  'aria-label': `Remove ${props.row.label}`,
                  'aria-disabled': props.pending || undefined,
                  onClick: () => {
                    if (!props.pending) props.remove();
                  },
                },
                '×',
              )
            : null,
        ],
      );
  },
});
