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
/** Verify the polished personal-to-pack UI application flow. */

import { describe, expect, jest, test } from '@jest/globals';
import { CubePackService } from '../../web/comfyui/ui/packs/CubePackService.js';
import { CubePromotionService } from '../../web/comfyui/ui/promotion/CubePromotionService.js';
import { CubeIdentityReconciler } from '../../web/comfyui/ui/graph/CubeIdentityReconciler.js';
import type { FormValues } from '../../web/comfyui/ui/dialogs/FormModal.js';

const PERSONAL_CUBE = {
  cube_id: 'local/personal/Text to Image.cube',
  name: 'Text to Image',
  display_name: 'Text to Image',
  description: 'Draft',
  version: '2.3.4',
  is_writable: true,
  supported_models: [],
};

describe('cube promotion service', () => {
  test('moves a personal cube after pack and sharing details are confirmed', async () => {
    const identityReconciler = { reconcile: jest.fn() };
    const cubeBrowser = {
      refresh: jest.fn(async () => {}),
      selectCube: jest.fn(),
    };
    const toast = { push: jest.fn() };
    const api = {
      promote: jest.fn(async (body: BodyInit | null) => {
        expect(JSON.parse(typeof body === 'string' ? body : '{}')).toEqual({
          source_cube_id: PERSONAL_CUBE.cube_id,
          destination: { owner: 'ExampleUser', repo: 'Example-Cubes' },
          name: 'Text to Image',
          target_model: 'SDXL',
          supported_models: ['SDXL', 'SD 1.5'],
          description: 'Ready to share',
        });
        return {
          response: { ok: true },
          data: {
            status: 'complete',
            version: '2.3.4',
            cube: {
              cube_id: 'ExampleUser/Example-Cubes/SDXL/Text to Image.cube',
              default_alias: 'SDXL/Text to Image',
            },
          },
        };
      }),
    };
    const service = new CubePromotionService({
      api,
      dialogs: {
        openForm: jest.fn(async () => ({
          name: 'Text to Image',
          target_model: 'SDXL',
          supported_models: 'SD 1.5',
          description: 'Ready to share',
        })),
      },
      toast,
      packService: {
        chooseWritablePack: jest.fn(async () => ({
          owner: 'ExampleUser',
          repo: 'Example-Cubes',
          repoRef: 'ExampleUser/Example-Cubes',
        })),
      },
      identityReconciler,
      cubeBrowser,
    });

    const result = await service.promote(PERSONAL_CUBE);

    expect(result!.status).toBe('complete');
    expect(identityReconciler.reconcile).toHaveBeenCalledWith({
      previousCubeId: PERSONAL_CUBE.cube_id,
      cubeId: 'ExampleUser/Example-Cubes/SDXL/Text to Image.cube',
      defaultAlias: 'SDXL/Text to Image',
    });
    expect(cubeBrowser.refresh).toHaveBeenCalledWith({ force: true });
    expect(cubeBrowser.selectCube).toHaveBeenCalledWith(
      'ExampleUser/Example-Cubes/SDXL/Text to Image.cube',
      { focus: false, silent: true },
    );
    expect(toast.push).toHaveBeenCalledWith(
      'success',
      'SugarCube moved to pack',
      expect.stringContaining('2.3.4'),
    );
  });

  test('rejects managed and imported cubes before opening promotion UI', async () => {
    const packService = { chooseWritablePack: jest.fn(async () => null) };
    const service = new CubePromotionService({
      packService,
      toast: { push: jest.fn() },
    });

    await expect(
      service.promote({ ...PERSONAL_CUBE, cube_id: 'ExampleUser/Pack/Demo.cube' }),
    ).resolves.toBeNull();
    expect(packService.chooseWritablePack).not.toHaveBeenCalled();
  });
});

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
        selectItem: jest.fn(async () => '__create_pack__'),
      },
      toast: { push: jest.fn() },
    });

    const pack = await service.chooseWritablePack();

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

describe('cube identity reconciler', () => {
  test('retargets live markers and managed groups after backend promotion', () => {
    const marker = {
      type: 'SugarCubes.CubeInput',
      widgets: [
        { name: 'cube_id', value: PERSONAL_CUBE.cube_id },
        { name: 'default_alias', value: 'Text to Image' },
      ],
    };
    const group = {
      properties: {
        sugarcubes: {
          managed: true,
          cube_id: PERSONAL_CUBE.cube_id,
          default_alias: 'Text to Image',
        },
      },
    };
    const graph = {
      _nodes: [marker],
      _groups: [group],
      setDirtyCanvas: jest.fn(),
    };
    const definitionStore = { invalidateCube: jest.fn() };
    const instanceManager = { scheduleRefresh: jest.fn() };
    const dirtyManager = { requestRefresh: jest.fn() };
    const reconciler = new CubeIdentityReconciler({
      adapter: { getApp: () => ({ graph }) },
      definitionStore,
      instanceManager,
      dirtyManager,
    });

    const result = reconciler.reconcile({
      previousCubeId: PERSONAL_CUBE.cube_id,
      cubeId: 'ExampleUser/Example-Cubes/SDXL/Text to Image.cube',
      defaultAlias: 'SDXL/Text to Image',
    });

    expect(result).toEqual({ markers: 1, groups: 1 });
    expect(marker.widgets[0].value).toBe('ExampleUser/Example-Cubes/SDXL/Text to Image.cube');
    expect(marker.widgets[1].value).toBe('SDXL/Text to Image');
    expect(group.properties.sugarcubes.cube_id).toBe(
      'ExampleUser/Example-Cubes/SDXL/Text to Image.cube',
    );
    expect(definitionStore.invalidateCube).toHaveBeenCalledWith(PERSONAL_CUBE.cube_id);
    expect(graph.setDirtyCanvas).toHaveBeenCalledWith(true, true);
  });
});
