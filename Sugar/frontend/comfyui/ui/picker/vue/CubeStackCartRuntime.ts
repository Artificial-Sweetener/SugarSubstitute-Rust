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
/** Mount and refresh one isolated Vue cart while preserving search and native focus. */
import { createApp, h, shallowRef } from 'vue';
import { CubeStackCart } from './CubeStackCart.js';
import type {
  CubeStackCartViewActions,
  CubeStackCartViewState,
  MountedCubeStackCartView,
} from '../CubeStackCartView.js';

/** Own one declarative application for the shared modal's content lifetime. */
export function mountCubeStackCart(
  element: HTMLElement,
  initial: CubeStackCartViewState,
  actions: CubeStackCartViewActions,
): MountedCubeStackCartView {
  const state = shallowRef(initial);
  const app = createApp({ render: () => h(CubeStackCart, { state: state.value, actions }) });
  app.mount(element);
  return {
    update: (next) => {
      state.value = next;
    },
    dispose: () => app.unmount(),
    focusSearch: () => element.querySelector<HTMLElement>('.sc-cube-stack-cart__search')?.focus(),
  };
}
