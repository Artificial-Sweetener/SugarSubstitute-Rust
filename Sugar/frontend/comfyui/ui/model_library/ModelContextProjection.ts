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
/** Translate model policy into universal rows while retaining the captured action owner. */
import type { ContextMenuEntry } from '../menus/ContextMenu.js';
import type { ModelContextEntry } from './ModelContextMenu.js';
import { createModelProviderIcon } from './vue/ModelProviderIcon.js';

export interface ModelContextContribution {
  readonly fieldName: string;
  readonly entries: readonly ContextMenuEntry[];
}

/** Share identical navigation, icons and commands across field and node-title entry points. */
export function projectModelContext(
  items: readonly ModelContextEntry[],
  command: (entry: ModelContextEntry) => void,
): readonly ContextMenuEntry[] {
  return items.map((item) => ({
    label: item.label,
    disabled: !item.enabled,
    ...(item.separator ? { separator: true } : {}),
    ...(item.checked === undefined ? {} : { checked: item.checked }),
    ...(item.children ? { children: projectModelContext(item.children, command) } : {}),
    ...(item.providerIcon
      ? { icon: (document: Document) => createModelProviderIcon(document, item.providerIcon ?? '') }
      : {}),
    select: () => {
      if (item.url) window.open(item.url, '_blank', 'noopener,noreferrer');
      else command(item);
    },
  }));
}
