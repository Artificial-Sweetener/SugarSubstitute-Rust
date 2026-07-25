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
/** Verify proximity routing preserves Comfy's natively compiled Cube payload. */

import { jest } from '@jest/globals';
import { CubePromptPipeline } from '../../frontend/comfyui/ui/cube/execution/CubePromptPipeline.js';

test('applies dotted routing directly to Comfy native subgraph output', async () => {
  const applyProximity = jest.fn(() => ({ output: { cube: { connected: true } } }));
  const adaptCubeOutputs = jest.fn((payload: unknown) => ({
    ...(payload as object),
    output: { cube: { connected: true }, sink: { class_type: 'SugarCubes.CubeOutput' } },
  }));
  const pipeline = new CubePromptPipeline({
    applyProximity,
    adaptCubeOutputs,
  });

  await expect(pipeline.transform({ output: { root: {} } })).resolves.toEqual({
    output: { cube: { connected: true }, sink: { class_type: 'SugarCubes.CubeOutput' } },
  });
  expect(applyProximity).toHaveBeenCalledWith({ output: { root: {} } });
  expect(adaptCubeOutputs).toHaveBeenCalledWith({ output: { cube: { connected: true } } });
});
