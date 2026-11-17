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
/** Mount the picker's model wall for an owner that supplies its own candidates and selection. */
import { createApp, h, shallowRef } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import { modelWallNeighbor } from '../ModelPickerNavigation.js';
import { ModelPickerWall } from './ModelPickerWall.js';

/** Arrow keys that move the current model across the wall's rows. */
export type ModelGalleryArrow = 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight';

export interface ModelGalleryEvents {
  /** The person chose the model at this position of the shown order. */
  activate(index: number): void;
}

export interface MountedModelGallery {
  /** Show these models in order with one current model. */
  update(items: readonly ModelCatalogItem[], active: number): void;
  /** The model one arrow key away from `active` on the wall as it is laid out now. */
  neighbor(active: number, key: ModelGalleryArrow): number;
  dispose(): void;
}

let mountIdentity = 0;

/** Bind one wall to a scoped Vue application; the caller keeps the candidates and the selection. */
export function mountModelGallery(
  element: HTMLElement,
  events: ModelGalleryEvents,
): MountedModelGallery {
  const items = shallowRef<readonly ModelCatalogItem[]>([]);
  const active = shallowRef(0);
  // The wall reports its measured width; until then it lays out at its own initial width.
  let width = 540;
  const identity = `sc-model-gallery-${String(++mountIdentity)}`;
  const app = createApp({
    render: () =>
      h(ModelPickerWall, {
        items: items.value,
        selected: '',
        active: active.value,
        optionPrefix: identity,
        onActivate: (value: string) => {
          const index = items.value.findIndex((item) => item.value === value);
          if (index >= 0) events.activate(index);
        },
        onGeometry: (measured: number) => {
          width = measured;
        },
      }),
  });
  app.config.idPrefix = identity;
  app.mount(element);
  return {
    update: (next, current) => {
      items.value = next;
      active.value = current;
    },
    neighbor: (current, key) => modelWallNeighbor(items.value, width, current, key),
    dispose: () => app.unmount(),
  };
}
