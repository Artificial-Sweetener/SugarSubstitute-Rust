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
/** Verify the shared Comfy root mutation boundary preserves host ownership. */

import { jest } from '@jest/globals';
import { ComfyCubeRootMutationAdapter } from '../../../frontend/comfyui/ui/cube/ComfyCubeRootMutationAdapter.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

test('binds root mutations and narrows dynamic link values', () => {
  const node = { id: 'cube' } as CubeNode;
  const add = jest.fn();
  const remove = jest.fn();
  const getNodeById = jest.fn(() => node);
  const getLink = jest.fn((id: unknown) => (id === 1 ? { id: 1 } : 'invalid'));
  const adapter = new ComfyCubeRootMutationAdapter({ add, remove, getNodeById, getLink });

  adapter.add(node);
  adapter.remove(node);

  expect(add).toHaveBeenCalledWith(node);
  expect(remove).toHaveBeenCalledWith(node);
  expect(adapter.getNodeById('cube')).toBe(node);
  expect(adapter.getLink(1)).toEqual({ id: 1 });
  expect(adapter.getLink(2)).toBeNull();
});
