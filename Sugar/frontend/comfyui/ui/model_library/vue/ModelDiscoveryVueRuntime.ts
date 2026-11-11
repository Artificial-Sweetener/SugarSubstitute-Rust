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
/** Mount the reference discovery gallery without importing Vue into dialog orchestration. */

import { createApp, h, shallowRef } from 'vue';
import { ModelDiscoveryGallery } from './ModelDiscoveryGallery.js';
import type {
  ModelDiscoveryViewActions,
  ModelDiscoveryViewState,
  MountedModelDiscoveryView,
} from '../ModelDiscoveryView.js';

/** Bind one gallery and its contained credential prompt to explicit scoped state updates. */
export function mountModelDiscoveryView(
  element: HTMLElement,
  initial: ModelDiscoveryViewState,
  actions: ModelDiscoveryViewActions,
): MountedModelDiscoveryView {
  const state = shallowRef(initial);
  const app = createApp({
    render: () => h(ModelDiscoveryGallery, { state: state.value, actions }),
  });
  app.mount(element);
  return {
    update: (next) => {
      state.value = next;
    },
    dispose: () => app.unmount(),
  };
}
