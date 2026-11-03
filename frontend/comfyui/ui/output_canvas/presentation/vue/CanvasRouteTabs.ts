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
/** Render the Input/Output route as a tab pair in the canvas tab controls. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasRouteId, CanvasRouteState } from '../../domain/CanvasRoute.js';

const ROUTES: readonly CanvasRouteId[] = ['input', 'output'];

/** Keep labels literal and disable Input with its reason while the workflow has no surfaces. */
export const CanvasRouteTabs = defineComponent({
  name: 'CanvasRouteTabs',
  props: {
    state: { type: Object as PropType<CanvasRouteState>, required: true },
    choose: { type: Function as PropType<(route: CanvasRouteId) => void>, required: true },
  },
  /** Follow tab-list keyboard conventions over the enabled routes. */
  setup(props) {
    /** Move between enabled routes with arrow, Home and End keys. */
    function navigate(event: KeyboardEvent): void {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      event.stopPropagation();
      const next =
        event.key === 'Home' || (event.key !== 'End' && props.state.route === 'output')
          ? 'input'
          : 'output';
      if (next === 'input' && !props.state.inputAvailable) return;
      const target = event.currentTarget;
      if (target instanceof HTMLElement)
        target.querySelector<HTMLButtonElement>(`[data-route="${next}"]`)?.focus();
      props.choose(next);
    }
    return () =>
      h('div', { class: 'sc-output-tabstrip sc-output-route' }, [
        h(
          'nav',
          {
            class: 'sc-output-tabs',
            role: 'tablist',
            'aria-label': props.state.labels.group,
            onKeydown: navigate,
          },
          ROUTES.map((route) => {
            const selected = props.state.route === route,
              disabled = route === 'input' && !props.state.inputAvailable;
            return h(
              'button',
              {
                key: route,
                type: 'button',
                role: 'tab',
                'data-route': route,
                'aria-selected': selected,
                tabIndex: selected ? 0 : -1,
                disabled,
                title: disabled ? props.state.reason : undefined,
                onClick: () => {
                  if (!disabled && !selected) props.choose(route);
                },
              },
              props.state.labels[route],
            );
          }),
        ),
      ]);
  },
});
