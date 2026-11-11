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
/** Verify claimed identity and writable pack authoring. */

import { describe, expect, jest, test } from '@jest/globals';
import { CubePackService } from '../../../frontend/comfyui/ui/packs/CubePackService.js';
import type { FormValues } from '../../../frontend/comfyui/ui/dialogs/FormModal.js';

describe('cube pack service', () => {
  test('claims an owner and creates the first writable pack in one flow', async () => {
    const formResponses: FormValues[] = [{ owner: 'ExampleUser' }, { repo: 'Example-Cubes' }];
    const openForm = jest.fn(async () => formResponses.shift() ?? null);
    const api = {
      listCubePacks: jest.fn(async () => ({
        response: { ok: true },
        data: { repos: [], identity_policy: { claimed_github_owner: '' } },
      })),
      updateIdentityPolicy: jest.fn(async (_payload: BodyInit | null) => ({
        response: { ok: true },
        data: { claimed_github_owner: 'ExampleUser' },
      })),
      createAuthoringCubePack: jest.fn(async (_payload: BodyInit | null) => ({
        response: { ok: true },
        data: {
          repo: {
            owner: 'ExampleUser',
            repo: 'Example-Cubes',
            enabled: true,
            is_writable: true,
          },
        },
      })),
    };
    const service = new CubePackService({
      api,
      dialogs: {
        openForm,
      },
      toast: { push: jest.fn() },
    });

    const pack = await service.createAuthoringPackForClaimedOwner();

    expect(pack).toEqual({
      owner: 'ExampleUser',
      repo: 'Example-Cubes',
      repoRef: 'ExampleUser/Example-Cubes',
    });
    expect(JSON.parse(String(api.updateIdentityPolicy.mock.calls[0][0]))).toEqual({
      claimed_github_owner: 'ExampleUser',
    });
    expect(JSON.parse(String(api.createAuthoringCubePack.mock.calls[0][0]))).toEqual({
      owner: 'ExampleUser',
      repo: 'Example-Cubes',
      enabled: true,
    });
  });
});
