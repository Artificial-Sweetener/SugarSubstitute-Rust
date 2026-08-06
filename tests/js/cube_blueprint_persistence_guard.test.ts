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
/** Verify the final persistence boundary excludes Sugar-marked Blueprints. */

import { jest } from '@jest/globals';
import { CubeBlueprintPersistenceGuard } from '../../frontend/comfyui/ui/affordance/CubeBlueprintPersistenceGuard.js';

test('blocks Sugar documents in the Blueprint namespace and preserves ordinary writes', () => {
  const storeUserData = jest.fn((_path: string, _data: unknown) => Promise.resolve());
  const api = { storeUserData };
  const guard = new CubeBlueprintPersistenceGuard(api);
  guard.install();

  expect(() =>
    api.storeUserData('subgraphs/cube.json', '{"properties":{"sugarcubes_kind":"cube"}}'),
  ).toThrow(/versioned \.cube documents/);
  expect(api.storeUserData('subgraphs/ordinary.json', '{"nodes":[]}')).toBeInstanceOf(Promise);
  expect(storeUserData).toHaveBeenCalledTimes(1);
  guard.dispose();
  expect(api.storeUserData).toBe(storeUserData);
});
