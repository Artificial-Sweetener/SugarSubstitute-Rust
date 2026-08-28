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
/** Verify picker lifecycle closes Add Cube state during renderer transitions. */

import { expect, jest, test } from '@jest/globals';
import type { CubeAddMenuController } from '../../../frontend/comfyui/ui/picker/CubeAddMenuController.js';
import type { ComfyCubePickerCreationAdapter } from '../../../frontend/comfyui/ui/picker/ComfyCubePickerCreationAdapter.js';
import type { ComfyCubePickerDefinitionAdapter } from '../../../frontend/comfyui/ui/picker/ComfyCubePickerDefinitionAdapter.js';
import type { ComfyCubePickerResultPresenter } from '../../../frontend/comfyui/ui/picker/ComfyCubePickerResultPresenter.js';
import { CubePickerHostIntegration } from '../../../frontend/comfyui/ui/picker/CubePickerHostIntegration.js';

test('subscribes once and closes the Add Cube menu whenever node style changes', () => {
  const renderer = { changed: null as (() => void) | null };
  const close = jest.fn();
  const install = jest.fn();
  const creationInstall = jest.fn();
  const integration = new CubePickerHostIntegration({
    definitions: {} as ComfyCubePickerDefinitionAdapter,
    creation: { install: creationInstall } as unknown as ComfyCubePickerCreationAdapter,
    results: { install, schedule: jest.fn() } as unknown as ComfyCubePickerResultPresenter,
    addMenu: { close, open: jest.fn() } as unknown as CubeAddMenuController,
    rendererChanges: {
      subscribe(listener) {
        renderer.changed = listener;
        return jest.fn();
      },
    },
    logger: { error: jest.fn() },
    reportError: jest.fn(),
  });

  integration.activate();
  integration.activate();
  renderer.changed?.();

  expect(install).toHaveBeenCalledTimes(2);
  expect(creationInstall).toHaveBeenCalledTimes(2);
  expect(close).toHaveBeenCalledTimes(1);
});
