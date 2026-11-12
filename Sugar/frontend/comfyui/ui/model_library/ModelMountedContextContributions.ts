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
/** Project mounted field actions and one eligible Cube policy menu independently of mount order. */
import type { ContextMenuEntry } from '../menus/ContextMenu.js';
import type { ModelCatalogSnapshot } from './ModelCatalog.js';
import type { ModelContextContribution } from './ModelContextProjection.js';
import type { ModelPickerBinding } from './ModelPickerBinding.js';
import { modelScopeMenu } from './ModelScopeMenus.js';

interface MountedContextField {
  readonly root: HTMLElement;
  readonly inputName: string;
  readonly binding: ModelPickerBinding;
  readonly picker: { contextEntries(): readonly ContextMenuEntry[] };
}

/** Attach Cube scope to the first eligible model field rather than an unrelated first mount. */
export function mountedModelContextContributions(
  mounts: Iterable<MountedContextField>,
  root: HTMLElement,
  app: unknown,
  snapshot: ModelCatalogSnapshot | null,
  changed: () => void,
): readonly ModelContextContribution[] {
  let scopeAdded = false;
  return [...mounts]
    .filter((mount) => mount.root === root)
    .map((mount) => {
      const scope =
        !scopeAdded && snapshot ? modelScopeMenu(app, mount.binding, snapshot, changed) : [];
      if (scope.length) scopeAdded = true;
      return { fieldName: mount.inputName, entries: [...mount.picker.contextEntries(), ...scope] };
    });
}
