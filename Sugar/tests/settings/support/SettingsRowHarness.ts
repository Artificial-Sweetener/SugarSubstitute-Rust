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
/** Draw the settings rows features contribute, exactly as the settings workspace draws them. */
import { settingsCatalogFor } from '../../../frontend/comfyui/ui/settings/workspace/SettingsCatalog.js';
import {
  renderSettingsRow,
  type SettingsCardView,
} from '../../../frontend/comfyui/ui/settings/workspace/SettingsCards.js';

/** The drawn rows of one app, and their teardown. */
export interface DrawnRows {
  readonly root: HTMLElement;
  /** The drawn card of one row id. */
  card(id: string): HTMLElement;
  /** The control of one row matching `selector`. */
  control<T extends Element = HTMLElement>(id: string, selector: string): T;
  dispose(): void;
}

/** Draw each contributed row of `ids` into a root attached to the document. */
export function drawRows(
  app: unknown,
  ids: readonly string[],
  report: (error: unknown) => void,
): DrawnRows {
  const catalog = settingsCatalogFor(app);
  const root = document.createElement('div');
  const views: SettingsCardView[] = [];
  for (const id of ids) {
    const row = catalog.row(id);
    if (!row) throw new Error(`Row ${id} was not contributed.`);
    const view = renderSettingsRow(document, row, report);
    views.push(view);
    root.append(view.element);
  }
  document.body.append(root);
  const card = (id: string): HTMLElement => {
    const element = root.querySelector<HTMLElement>(`[data-row="${id}"]`);
    if (!element) throw new Error(`Row ${id} is not drawn.`);
    return element;
  };
  return {
    root,
    card,
    control: <T extends Element = HTMLElement>(id: string, selector: string): T => {
      const element = card(id).querySelector<T>(selector);
      if (!element) throw new Error(`Row ${id} has no ${selector}.`);
      return element;
    },
    dispose: () => {
      for (const view of views) view.dispose();
      root.remove();
    },
  };
}

/** Drain promise reactions and timers deterministically. */
export async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}
