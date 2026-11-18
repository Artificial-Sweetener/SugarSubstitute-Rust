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
/** Open the standalone picker popup in its own scoped Vue application. */
import { createApp, h } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import type { ModelPickerRectangle } from '../ModelPickerPlacement.js';
import { ModelChooser } from './ModelChooser.js';

export interface ModelChooserRequest {
  readonly items: readonly ModelCatalogItem[];
  /** Names the popup and its search box. */
  readonly label: string;
  /** The viewport point or rectangle the popup opens beside. */
  readonly anchor: ModelPickerRectangle;
}

export interface OpenModelChooser {
  /** The popup surface, for the picker's appearance owner. */
  readonly popup: HTMLElement | null;
  /** Close without choosing; a chooser that already settled is unaffected. */
  dismiss(): void;
}

let mountIdentity = 0;

/**
 * Show the chooser until a model is chosen or it is dismissed, then report once.
 *
 * `settled` receives the chosen model or null; the popup is gone before it is called.
 */
export function openModelChooser(
  document: Document,
  request: ModelChooserRequest,
  settled: (item: ModelCatalogItem | null) => void,
): OpenModelChooser {
  const container = document.createElement('div');
  document.body.append(container);
  let open = true;
  /** Unmount first, so the caller may move focus or edit without the popup in the way. */
  const settle = (item: ModelCatalogItem | null): void => {
    if (!open) return;
    open = false;
    app.unmount();
    container.remove();
    settled(item);
  };
  const app = createApp({
    render: () =>
      h(ModelChooser, {
        ...request,
        onChoose: (item: ModelCatalogItem) => settle(item),
        onDismiss: () => settle(null),
      }),
  });
  app.config.idPrefix = `sc-model-chooser-${String(++mountIdentity)}`;
  app.mount(container);
  return {
    popup: container.querySelector<HTMLElement>('.sc-model-popup'),
    dismiss: () => settle(null),
  };
}
