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
/** Mount version-family Vue rendering through a typed host-independent view boundary. */
import { createApp, h, shallowRef } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import type { ModelRelease } from '../ModelReleases.js';
import { ModelVersionFamily } from './ModelVersionFamily.js';

export interface ModelVersionViewState {
  readonly versions: readonly ModelRelease[];
  readonly selected: string;
  readonly busy: boolean;
}
export interface MountedModelVersionView {
  update(state: ModelVersionViewState): void;
  dispose(): void;
}

/** Bind an isolated chronology to explicit state updates without importing Vue into dialog coordination. */
export function mountModelVersionView(
  element: HTMLElement,
  item: ModelCatalogItem,
  initial: ModelVersionViewState,
  onSelect: (version: ModelRelease) => void,
): MountedModelVersionView {
  const state = shallowRef(initial);
  const app = createApp({
    render: () => h(ModelVersionFamily, { item, ...state.value, onSelect }),
  });
  app.mount(element);
  return {
    update: (next) => {
      state.value = next;
    },
    dispose: () => app.unmount(),
  };
}
