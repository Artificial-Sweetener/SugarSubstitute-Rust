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
/** Contribute the six portable canvas dimension commands to the shared width/height menu. */
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import {
  CanvasDimensionScaleMenu,
  type ExecuteCanvasDimension,
} from './CanvasDimensionScaleMenu.js';
import type { CanvasDimensionAction } from '../application/CanvasDimensionCommands.js';

/** Describe commands without introducing controls, style overrides or another menu implementation. */
export function canvasDimensionMenuEntries(
  available: boolean,
  execute: ExecuteCanvasDimension,
  run: (operation: Promise<void>) => void,
): ContextMenuEntry[] {
  if (!available) return [];
  const actions: readonly [CanvasDimensionAction, string][] = [
    ['generation-from-canvas', 'Set size from canvas'],
    ['canvas-from-generation', 'Resize canvas to size'],
    ['generation-from-selection', 'Set size from selection'],
    ['generation-from-canvas-scaled', 'Set size from canvas with scaling'],
    ['canvas-from-generation-scaled', 'Resize canvas to size with scaling'],
    ['generation-from-selection-scaled', 'Set size from selection with scaling'],
  ];
  return [
    { label: '', separator: true },
    ...actions.map(([action, label]) => {
      if (action.endsWith('-scaled')) {
        const menu = new CanvasDimensionScaleMenu(action, execute, run);
        return { label, children: menu.entries(), opened: menu.opened };
      }
      return {
        label,
        select: () => {
          run(execute(action));
        },
      };
    }),
  ];
}
