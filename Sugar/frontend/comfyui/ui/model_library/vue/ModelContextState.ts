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
/** Present the shared model menu at the pointer for the Vue model surfaces. */
import { h, onBeforeUnmount, ref, shallowRef } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import type { ModelContextEntry } from '../ModelContextMenu.js';
import type { ModelPickerSession } from '../ModelPickerSession.js';
import { ModelMenu, type ModelContextOptions } from '../ModelMenu.js';
import { ModelContextMenuView } from './ModelContextMenuView.js';

/** Reuse one model menu owner for banners, tiles and update-icon menus without owning model values. */
export function useModelContext(
  options: ModelContextOptions,
  disabled: () => boolean,
  beforeFamily: () => void,
  session: ModelPickerSession | null,
) {
  const menu = ref<{ item: ModelCatalogItem; x: number; y: number; updateOnly: boolean } | null>(
    null,
  );
  const owner = new ModelMenu(options, session, disabled, () => {
    menu.value = null;
    beforeFamily();
  });
  const proposals = shallowRef(owner.proposals);
  const unsubscribe = owner.subscribe(() => {
    proposals.value = owner.proposals;
  });
  /** Open a conditional menu at the pointer without committing any native widget callback. */
  const show = (item: ModelCatalogItem, event: MouseEvent, updateOnly = false): void => {
    event.preventDefault();
    event.stopPropagation();
    if (!owner.entries(item, updateOnly).length) return;
    menu.value = { item, x: event.clientX, y: event.clientY, updateOnly };
  };
  onBeforeUnmount(() => {
    menu.value = null;
    unsubscribe();
    owner.dispose();
  });
  /** Render the transient menu with a captured target that survives its close event. */
  const render = () => {
    const current = menu.value;
    return current
      ? h(ModelContextMenuView, {
          entries: owner.entries(current.item, current.updateOnly),
          x: current.x,
          y: current.y,
          onClose: () => {
            menu.value = null;
          },
          onAction: (entry: ModelContextEntry) => owner.command(current.item, entry),
        })
      : null;
  };
  return {
    menu,
    show,
    family: (item: ModelCatalogItem) => owner.family(item),
    offer: (item: ModelCatalogItem) => owner.offer(item),
    render,
    proposals,
    /** Capture the selected model without creating another metadata or command owner. */
    project: (item: ModelCatalogItem) => owner.project(item),
  };
}
