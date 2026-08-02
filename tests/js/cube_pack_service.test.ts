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
/** Verify saved Cube editability follows local and writable-pack ownership. */

import { jest } from '@jest/globals';
import { CubePackService } from '../../frontend/comfyui/ui/packs/CubePackService.js';

test('allows local Cubes and only Cube Packs reported writable by the backend', async () => {
  const listCubePacks = jest.fn(async () => ({
    response: { ok: true, statusText: '' },
    data: {
      repos: [
        { owner: 'Me', repo: 'Authoring', enabled: true, is_writable: true },
        { owner: 'Else', repo: 'ReadOnly', enabled: true, is_writable: false },
      ],
      identity_policy: {},
    },
  }));
  const service = new CubePackService({
    api: {
      listCubePacks,
      updateIdentityPolicy: async () => ({ response: { ok: true }, data: {} }),
      createAuthoringCubePack: async () => ({ response: { ok: true }, data: {} }),
    } as never,
  });

  await expect(service.canWriteCube('local/personal/SDXL/mine.cube')).resolves.toBe(true);
  await expect(service.canWriteCube('me/authoring/SDXL/mine.cube')).resolves.toBe(true);
  await expect(service.canWriteCube('else/readonly/SDXL/theirs.cube')).resolves.toBe(false);
});

test('lists only writable packs owned by the claimed author for the save modal', async () => {
  const service = new CubePackService({
    api: {
      listCubePacks: async () => ({
        response: { ok: true, statusText: '' },
        data: {
          repos: [
            { owner: 'Me', repo: 'Authoring', enabled: true, is_writable: true },
            { owner: 'Else', repo: 'AlsoWritable', enabled: true, is_writable: true },
            { owner: 'Me', repo: 'Disabled', enabled: false, is_writable: true },
          ],
          identity_policy: { claimed_github_owner: 'me' },
        },
      }),
      updateIdentityPolicy: async () => ({ response: { ok: true }, data: {} }),
      createAuthoringCubePack: async () => ({ response: { ok: true }, data: {} }),
    } as never,
  });

  await expect(service.listAuthoringPacks()).resolves.toEqual([
    { owner: 'Me', repo: 'Authoring', repoRef: 'Me/Authoring' },
  ]);
});
