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
/** Mount picker-owned Vue presentation without changing Comfy's Vue application. */
import { createApp, h, shallowRef } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import type { ModelPickerBinding } from '../ModelPickerBinding.js';
import type { ModelPickerSession } from '../ModelPickerSession.js';
import { ModelPickerField } from './ModelPickerField.js';
import type { ModelPickerActions, ModelPickerFeedback } from '../ModelPickerActions.js';
import type { ModelUpdatePort } from '../ModelUpdates.js';
import type { ModelDetectorPort } from '../ModelDetectors.js';
import type { ModelOutputThumbnails } from '../ModelOutputThumbnails.js';
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import type { ModelCompatibilityContext } from '../ModelCompatibility.js';

export interface MountedModelPicker {
  contextEntries(): readonly ContextMenuEntry[];
  update(items: readonly ModelCatalogItem[]): void;
  updateDiscovery(action: (() => void) | null): void;
  dispose(): void;
}

let mountIdentity = 0;

/** Bind one field to a scoped Vue application with an explicit teardown contract. */
export function mountModelPicker(
  element: HTMLElement,
  binding: ModelPickerBinding,
  initialItems: readonly ModelCatalogItem[],
  actions: ModelPickerActions | null = null,
  feedback: ModelPickerFeedback | null = null,
  updates: ModelUpdatePort | null = null,
  detectors: ModelDetectorPort | null = null,
  outputs: ModelOutputThumbnails | null = null,
  compatibility: (() => ModelCompatibilityContext | null) | null = null,
  session: ModelPickerSession | null = null,
): MountedModelPicker {
  const items = shallowRef(initialItems);
  const discoveryAction = shallowRef<(() => void) | null>(null);
  let contextEntries: () => readonly ContextMenuEntry[] = () => [];
  const app = createApp({
    render: () =>
      h(ModelPickerField, {
        binding,
        items: items.value,
        actions,
        feedback,
        updates,
        detectors,
        outputs,
        compatibility,
        session,
        discoveryAction: discoveryAction.value,
        onContextReady: (read: () => readonly ContextMenuEntry[]) => {
          contextEntries = read;
        },
      }),
  });
  app.config.idPrefix = `sc-model-${String(++mountIdentity)}`;
  app.mount(element);
  return {
    contextEntries: () => contextEntries(),
    update: (value) => {
      items.value = value;
    },
    dispose: () => app.unmount(),
    updateDiscovery: (action) => {
      discoveryAction.value = action;
    },
  };
}
