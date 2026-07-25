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
/** Characterize native Cube creation orchestration without marker state. */

import { jest } from '@jest/globals';
import { CubeCreationService } from '../../frontend/comfyui/ui/create/CubeCreationService.js';
import type {
  AuthoredCube,
  AuthoredCubeIdentity,
} from '../../frontend/comfyui/ui/cube/ComfyCubeAuthoringAdapter.js';
import type { CreatePersonalCubeModalOptions } from '../../frontend/comfyui/ui/dialogs/CreatePersonalCubeModal.js';

describe('CubeCreationService', () => {
  test('confirms identity, converts natively, and saves the new Cube id', async () => {
    const authored = {
      node: {},
      subgraph: {},
      identity: {},
    } as unknown as AuthoredCube;
    const createFromSelection = jest.fn(() => authored);
    const validateSelection = jest.fn();
    const save = jest.fn(async () => {});
    const openCreatePersonalCube = jest.fn(async () => ({
      name: 'Detailer',
      defaultAlias: 'Detailer',
      cubeId: 'local/personal/Detailer.cube',
    }));
    const toast = { push: jest.fn() };
    const service = new CubeCreationService({
      getAuthoring: () => ({
        selectedCount: () => 3,
        validateSelection,
        createFromSelection,
      }),
      cubeSave: { save },
      cubeBrowser: {
        getCubes: () => [{ cube_id: 'local/personal/Existing.cube' }],
      },
      dialogs: { openCreatePersonalCube },
      toast,
      createInstanceId: () => 'instance-1',
    });

    await expect(service.startCreateCubeFromSelection()).resolves.toBe(authored);

    expect(validateSelection).toHaveBeenCalledTimes(1);
    expect(openCreatePersonalCube).toHaveBeenCalledWith(
      expect.objectContaining({
        candidate: expect.objectContaining({ nodeIds: [0, 1, 2] }),
        deriveIdentity: expect.any(Function),
      }),
    );
    expect(createFromSelection).toHaveBeenCalledWith({
      cubeId: 'local/personal/Detailer.cube',
      defaultAlias: 'Detailer',
      instanceId: 'instance-1',
      targetModel: '',
      supportedModels: [],
      description: '',
    });
    expect(save).toHaveBeenCalledWith({
      cubeIds: ['local/personal/Detailer.cube'],
    });
    expect(toast.push).toHaveBeenCalledWith(
      'success',
      'SugarCube created',
      'Detailer is now a native Cube.',
    );
  });

  test('does not open identity UI or convert when no nodes are selected', async () => {
    const createFromSelection = jest.fn((_identity: AuthoredCubeIdentity): AuthoredCube => {
      throw new Error('unexpected conversion');
    });
    const openCreatePersonalCube = jest.fn(
      async (_options: CreatePersonalCubeModalOptions) => null,
    );
    const save = jest.fn(async () => {});
    const toast = { push: jest.fn() };
    const service = new CubeCreationService({
      getAuthoring: () => ({
        selectedCount: () => 0,
        validateSelection: () => {
          throw new Error('Select at least one node to create a SugarCube.');
        },
        createFromSelection,
      }),
      cubeSave: { save },
      dialogs: { openCreatePersonalCube },
      toast,
    });

    await expect(service.startCreateCubeFromSelection()).resolves.toBeNull();

    expect(openCreatePersonalCube).not.toHaveBeenCalled();
    expect(createFromSelection).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
    expect(toast.push).toHaveBeenCalledWith(
      'error',
      'SugarCube creation failed',
      'Select at least one node to create a SugarCube.',
    );
  });

  test('does not open identity UI when the selection lacks Cube boundaries', async () => {
    const openCreatePersonalCube = jest.fn(
      async (_options: CreatePersonalCubeModalOptions) => null,
    );
    const createFromSelection = jest.fn((_identity: AuthoredCubeIdentity): AuthoredCube => {
      throw new Error('unexpected conversion');
    });
    const toast = { push: jest.fn() };
    const service = new CubeCreationService({
      getAuthoring: () => ({
        selectedCount: () => 2,
        validateSelection: () => {
          throw new Error('A SugarCube requires at least one graph input and one graph output.');
        },
        createFromSelection,
      }),
      cubeSave: { save: jest.fn(async () => {}) },
      dialogs: { openCreatePersonalCube },
      toast,
    });

    await expect(service.startCreateCubeFromSelection()).resolves.toBeNull();

    expect(openCreatePersonalCube).not.toHaveBeenCalled();
    expect(createFromSelection).not.toHaveBeenCalled();
    expect(toast.push).toHaveBeenCalledWith(
      'error',
      'SugarCube creation failed',
      'A SugarCube requires at least one graph input and one graph output.',
    );
  });

  test('leaves the graph untouched when identity confirmation is cancelled', async () => {
    const createFromSelection = jest.fn((_identity: AuthoredCubeIdentity): AuthoredCube => {
      throw new Error('unexpected conversion');
    });
    const save = jest.fn(async () => {});
    const service = new CubeCreationService({
      getAuthoring: () => ({
        selectedCount: () => 1,
        validateSelection: () => {},
        createFromSelection,
      }),
      cubeSave: { save },
      dialogs: {
        openCreatePersonalCube: jest.fn(async (_options: CreatePersonalCubeModalOptions) => null),
      },
    });

    await expect(service.startCreateCubeFromSelection()).resolves.toBeNull();

    expect(createFromSelection).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
  });
});
