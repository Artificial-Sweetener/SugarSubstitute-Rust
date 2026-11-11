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
/** Render the shared icon and its model overlay without replacing the Vue-owned capsule. */
import { defineComponent, h, watch, type PropType } from 'vue';
import {
  createResolvedCubeIconElement,
  type CubeIconModel,
} from '../../../core/CubeIconResolver.js';

/** Render validated assets and reference fallback initials through the existing icon authority. */
export const CubeStackIcon = defineComponent({
  name: 'SugarCubesStackIcon',
  props: {
    icon: { type: Object as PropType<CubeIconModel>, required: true },
    targetModel: { type: String, required: true },
  },
  setup(props) {
    let element: HTMLElement | null = null;
    const render = (): void => {
      if (element) {
        const icon = createResolvedCubeIconElement(element.ownerDocument, props.icon);
        element.replaceChildren(icon);
      }
    };
    watch(() => JSON.stringify(props.icon), render);
    const retainElement = (value: unknown): void => {
      const next = value instanceof HTMLElement ? value : null;
      if (element === next) return;
      element = next;
      render();
    };
    return () =>
      h('span', { class: 'sc-cube-stack-row__icon', 'aria-hidden': 'true' }, [
        h('span', { class: 'sc-cube-stack-row__asset', ref: retainElement }),
        props.targetModel
          ? h('span', { class: 'sc-cube-stack-row__model' }, props.targetModel)
          : null,
      ]);
  },
});
