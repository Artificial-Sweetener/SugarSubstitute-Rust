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
/** Verify graph-clear notification remains isolated at the dynamic Comfy API boundary. */

import { jest } from '@jest/globals';
import { notifyComfyGraphCleared } from '../../frontend/comfyui/ui/affordance/ComfyGraphClearNotifier.js';

test('dispatches the stable graph-cleared event when Comfy exposes it', () => {
  const dispatchCustomEvent = jest.fn();
  const logger = { warn: jest.fn() };
  notifyComfyGraphCleared({ dispatchCustomEvent }, logger);
  expect(dispatchCustomEvent).toHaveBeenCalledWith('graphCleared');
  expect(logger.warn).not.toHaveBeenCalled();
});

test('warns without failing when the host event boundary changes', () => {
  const logger = { warn: jest.fn() };
  expect(() => notifyComfyGraphCleared({}, logger)).not.toThrow();
  expect(logger.warn).toHaveBeenCalledWith(
    'SugarCubes: Comfy graph-cleared event integration is unavailable.',
  );
});
