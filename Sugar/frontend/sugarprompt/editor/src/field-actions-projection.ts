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
/** Literal adaptation owns neither native admission nor a resource lifetime. */
import type { ContextMenuItem } from './context-menu-contracts.js';
import type {
  FieldActionContext,
  FieldActionContribution,
  FieldActionEntry,
  FieldActionOutcome,
} from './field-actions-contracts.js';
import { captureRecord } from '../../services/src/saved-segments/capture.js';

/** Admit a closed host anchor before focus capture or publication, preserving reflection causes. */
export function captureFieldActionContext(value: FieldActionContext): FieldActionContext {
  const record = captureRecord(value, ['left', 'right', 'top', 'bottom']);
  const { left, right, top, bottom } = record;
  if (
    typeof left !== 'number' ||
    !Number.isFinite(left) ||
    typeof right !== 'number' ||
    !Number.isFinite(right) ||
    typeof top !== 'number' ||
    !Number.isFinite(top) ||
    typeof bottom !== 'number' ||
    !Number.isFinite(bottom) ||
    left > right ||
    top > bottom
  )
    throw new TypeError('invalid-field-action-anchor');
  return Object.freeze({ left, right, top, bottom });
}
/** Preserve semantic ordering and genuine leaf closures without exporting the internal leaf or editor state. */
export function projectFieldActions(
  items: readonly ContextMenuItem[],
  activate: (item: ContextMenuItem) => Promise<FieldActionOutcome>,
  release: () => void,
): FieldActionContribution {
  /** Recursively adapt only the prepared literal presentation. */
  function entry(item: ContextMenuItem): FieldActionEntry {
    if (item.kind === 'heading') return Object.freeze({ kind: item.kind, label: item.label });
    if (item.kind === 'separator') return Object.freeze({ kind: item.kind });
    if (item.kind === 'submenu')
      return Object.freeze({
        kind: item.kind,
        label: item.label,
        children: Object.freeze(item.children.map(entry)),
      });
    return Object.freeze({
      kind: 'action',
      label: item.label,
      enabled: item.kind !== 'disabled',
      ...('title' in item ? { title: item.title } : {}),
      ...(item.kind === 'rich-rendering' ? { checked: item.checked } : {}),
      activate: () => activate(item),
    });
  }
  return Object.freeze({ entries: Object.freeze(items.map(entry)), release });
}
