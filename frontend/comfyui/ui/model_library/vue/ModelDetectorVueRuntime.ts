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
/** Render the detector illustration chooser through the shared virtualized media wall. */
import { createApp, h, ref } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import { ModelPickerWall } from './ModelPickerWall.js';
import { modelWallNeighbor } from '../ModelPickerNavigation.js';
import { MODEL_DETECTOR_WALL_PROFILE } from '../ModelPickerLayout.js';

/** Keep modal keyboard focus, selected art and mouse activation within an isolated Vue lifetime. */
export function mountDetectorWall(
  element: HTMLElement,
  items: readonly ModelCatalogItem[],
  selected: string,
  onSelect: (asset: string) => void,
): () => void {
  const active = ref(
    Math.max(
      0,
      items.findIndex((item) => item.value === selected),
    ),
  );
  const width = ref(540);
  const app = createApp({
    render: () =>
      h(
        'div',
        {
          class: 'sc-model-detector-wall',
          tabindex: 0,
          role: 'group',
          'aria-label': 'Detector thumbnails',
          onKeydown: (event: KeyboardEvent) => {
            if (
              ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)
            ) {
              event.preventDefault();
              active.value = modelWallNeighbor(
                items,
                width.value,
                active.value,
                event.key,
                MODEL_DETECTOR_WALL_PROFILE,
              );
            } else if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault();
              const item = items[active.value];
              if (item) onSelect(item.value);
            }
          },
        },
        h(ModelPickerWall, {
          items,
          selected,
          active: active.value,
          optionPrefix: 'sc-detector',
          profile: MODEL_DETECTOR_WALL_PROFILE,
          onGeometry: (next: number) => {
            width.value = next;
          },
          onHighlight: (index: number) => {
            active.value = index;
          },
          onActivate: onSelect,
        }),
      ),
  });
  app.mount(element);
  element.querySelector<HTMLElement>('[tabindex="0"]')?.focus();
  return () => app.unmount();
}
