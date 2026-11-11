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
/** Mount one Vue stack rail behind a typed lifecycle shared by both appearance modes. */
import { createApp, h, shallowRef } from 'vue';
import { CubeStackRail } from './CubeStackRail.js';
import type {
  CubeStackViewState,
  CubeStackViewActions,
  MountedCubeStackView,
} from '../CubeStackView.js';

/** Refresh declarative presentation without replacing the application or its focused row controls. */
export function mountCubeStackView(
  element: HTMLElement,
  initial: CubeStackViewState,
  actions: CubeStackViewActions,
): MountedCubeStackView {
  const state = shallowRef(initial);
  const interactive = shallowRef(true);
  const app = createApp({
    render: () => h(CubeStackRail, { state: state.value, actions, interactive: interactive.value }),
  });
  app.mount(element);
  return {
    update: (next) => {
      state.value = next;
    },
    setInteractive: (enabled) => {
      interactive.value = enabled;
    },
    dispose: () => app.unmount(),
  };
}
