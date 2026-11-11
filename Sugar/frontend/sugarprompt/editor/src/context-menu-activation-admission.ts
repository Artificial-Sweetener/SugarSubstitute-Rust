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
/** Prepared enabled-leaf membership has one authority across all native context-menu domains. */
import type { EditorState } from '@codemirror/state';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { contextMenu, currentContextMenu, type ContextMenu } from './context-menu-state.js';

/** Admit only the exact current visible enabled leaf; mounted disposal/composition remain with NativeContextMenu. */
export function currentContextMenuAction(
  state: EditorState,
  menu: ContextMenu,
  item: ContextMenuItem,
): boolean {
  if (!currentContextMenu(state, menu) || state.field(contextMenu) !== menu) return false;
  const actions = menu.actions;
  return (
    (item.kind === 'text' && menu.surface === 'local' && menu.text.leaves.has(item)) ||
    item === menu.scheduleItem ||
    item === menu.presets.saveLeaf ||
    (item.kind === 'insert-segment' && menu.presets.insertLeaves.has(item)) ||
    (item === menu.renderingItem && (menu.surface === 'field' || actions?.kind !== 'inline')) ||
    item === menu.queueItem ||
    item === actions?.pageItem ||
    (item.kind === 'source-edit' &&
      (actions?.editLeaves.has(item) ||
        ((menu.diagnostic?.kind === 'duplicate-segment' || menu.diagnostic?.kind === 'spelling') &&
          menu.diagnostic.editLeaves.has(item)))) ||
    (item.kind === 'spelling-action' &&
      menu.diagnostic?.kind === 'spelling' &&
      (menu.diagnostic.ignoreLeaf === item || menu.diagnostic.addLeaf === item)) ||
    (item.kind === 'ignore-duplicate' &&
      menu.diagnostic?.kind === 'duplicate-segment' &&
      menu.diagnostic.ignoreLeaf === item) ||
    (actions?.kind === 'inline' && item.kind === 'action' && actions.metadataLeaves.has(item))
  );
}
