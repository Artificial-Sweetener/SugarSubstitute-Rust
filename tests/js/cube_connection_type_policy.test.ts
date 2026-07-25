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
/** Verify Cube connections delegate type semantics to Comfy through one policy. */

import { expect, jest, test } from '@jest/globals';

import { CubeConnectionTypePolicy } from '../../frontend/comfyui/ui/cube/connection/CubeConnectionTypePolicy.js';

test('delegates normalized types to Comfy and fails closed when the host throws', () => {
  const isValidConnection = jest.fn((output: unknown, input: unknown) => {
    if (input === 'BROKEN') throw new Error('host failed');
    return output === input || output === '*';
  });
  const logger = { debug: jest.fn() };
  const policy = new CubeConnectionTypePolicy({
    getLiteGraph: () => ({ isValidConnection }),
    logger,
  });

  expect(policy.accepts(' image ', 'IMAGE')).toBe(true);
  expect(policy.accepts(null, 'LATENT')).toBe(true);
  expect(policy.accepts('IMAGE', 'LATENT')).toBe(false);
  expect(policy.accepts('IMAGE', 'BROKEN')).toBe(false);
  expect(logger.debug).toHaveBeenCalledTimes(1);
});
