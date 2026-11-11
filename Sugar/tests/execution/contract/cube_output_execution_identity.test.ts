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
/** Lock stable Cube output identities before execution ownership moves server-side. */

import {
  buildCubeOutputExecutionId,
  isCubeOutputExecutionId,
} from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';

test('builds and recognizes deterministic prompt-only output identities', () => {
  const identity = buildCubeOutputExecutionId('cube:stable-instance', 2);

  expect(identity).toBe('__sugarcubes_cube_output__:cube:stable-instance:2');
  expect(isCubeOutputExecutionId(identity)).toBe(true);
  expect(isCubeOutputExecutionId('__sugarcubes_cube_output__:cube:stable-instance:-1')).toBe(false);
});
