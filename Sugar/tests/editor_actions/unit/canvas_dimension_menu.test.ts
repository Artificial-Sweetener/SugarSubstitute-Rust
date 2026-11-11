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
/** Prove canvas dimension actions reuse shared menus and stay absent for incapable canvases. */
import { jest } from '@jest/globals';
import { canvasDimensionMenuEntries } from '../../../frontend/comfyui/ui/editor_actions/presentation/CanvasDimensionMenu.js';
import type { CanvasDimensionAction } from '../../../frontend/comfyui/ui/editor_actions/application/CanvasDimensionCommands.js';

test('contributes exactly six explicit actions without discovering a graph or executing on open', async () => {
  const execute = jest
    .fn<(action: CanvasDimensionAction) => Promise<void>>()
    .mockResolvedValue(undefined);
  const pending: Promise<void>[] = [];
  expect(
    canvasDimensionMenuEntries(false, execute, (operation) => {
      pending.push(operation);
    }),
  ).toEqual([]);
  const entries = canvasDimensionMenuEntries(true, execute, (operation) => {
    pending.push(operation);
  });
  expect(entries).toHaveLength(7);
  expect(execute).not.toHaveBeenCalled();
  const releases: (() => void)[] = [];
  for (const entry of entries) {
    if (entry.opened)
      releases.push(
        entry.opened(
          () => {},
          () => {},
        ),
      );
    else entry.select?.();
  }
  await Promise.all(pending);
  releases.forEach((release) => release());
  expect(execute.mock.calls.map(([action]) => action)).toEqual([
    'generation-from-canvas',
    'canvas-from-generation',
    'generation-from-selection',
    'generation-from-canvas-scaled',
    'canvas-from-generation-scaled',
    'generation-from-selection-scaled',
  ]);
});
