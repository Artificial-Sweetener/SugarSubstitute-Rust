//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify every Cube save entry point receives the same complete authoring session. */

import { jest } from '@jest/globals';
import type {
  CubeAuthoringDialogOptions,
  CubeAuthoringValues,
} from '../../../frontend/comfyui/ui/create/CubeAuthoringDialog.js';
import { CubeAuthoringService } from '../../../frontend/comfyui/ui/create/CubeAuthoringService.js';

describe('CubeAuthoringService', () => {
  test('supplies concrete local and author-pack destinations to first save', async () => {
    const openCubeAuthoring = jest.fn(
      async (options: CubeAuthoringDialogOptions): Promise<CubeAuthoringValues> => {
        expect(options.destinations).toEqual([
          expect.objectContaining({
            key: 'local/personal',
            destination: { kind: 'local' },
          }),
          expect.objectContaining({
            key: 'pack/artist/cubes',
            label: 'cubes',
            detail: 'artist',
            destination: {
              kind: 'pack',
              owner: 'artist',
              repo: 'cubes',
              repoRef: 'artist/cubes',
            },
          }),
          expect.objectContaining({ action: 'create-pack' }),
        ]);
        const pack = options.destinations?.[1]?.destination;
        if (!pack || !options.deriveIdentity) throw new Error('Missing authoring destination.');
        const identity = options.deriveIdentity('Shared', 'SDXL', pack);
        return {
          ...identity,
          targetModel: 'SDXL',
          supportedModels: ['SDXL'],
          description: '',
          destination: pack,
        };
      },
    );
    const service = new CubeAuthoringService({
      browser: {
        getCubes: () => [],
        getModelSuggestions: () => ['SDXL', 'Flux .1 D'],
      },
      dialogs: { openCubeAuthoring },
      packService: {
        listAuthoringPacks: jest.fn(async () => [
          { owner: 'artist', repo: 'cubes', repoRef: 'artist/cubes' },
        ]),
        createAuthoringPackForClaimedOwner: jest.fn(async () => null),
      },
    });

    await expect(
      service.openFirstSave({ defaultAlias: 'Shared', targetModel: 'SDXL' }),
    ).resolves.toEqual(expect.objectContaining({ cubeId: 'artist/cubes/SDXL/Shared.cube' }));
    expect(openCubeAuthoring).toHaveBeenCalledWith(
      expect.objectContaining({ modelSuggestions: ['SDXL', 'Flux .1 D'] }),
    );
  });

  test('creates a pack inside the active first-save session', async () => {
    const openCubeAuthoring = jest.fn(
      async (options: CubeAuthoringDialogOptions): Promise<null> => {
        await expect(options.onCreateDestination?.()).resolves.toEqual({
          key: 'pack/artist/new-cubes',
          label: 'new-cubes',
          detail: 'artist',
          destination: {
            kind: 'pack',
            owner: 'artist',
            repo: 'new-cubes',
            repoRef: 'artist/new-cubes',
          },
        });
        return null;
      },
    );
    const createAuthoringPackForClaimedOwner = jest.fn(async () => ({
      owner: 'artist',
      repo: 'new-cubes',
      repoRef: 'artist/new-cubes',
    }));
    const service = new CubeAuthoringService({
      dialogs: { openCubeAuthoring },
      packService: {
        listAuthoringPacks: jest.fn(async () => []),
        createAuthoringPackForClaimedOwner,
      },
    });

    await service.openFirstSave({});
    expect(createAuthoringPackForClaimedOwner).toHaveBeenCalledTimes(1);
  });

  test('locks an existing Cube to the destination encoded by its canonical identity', async () => {
    const openCubeAuthoring = jest.fn(async () => null);
    const service = new CubeAuthoringService({
      dialogs: { openCubeAuthoring },
      packService: {
        listAuthoringPacks: jest.fn(async () => []),
        createAuthoringPackForClaimedOwner: jest.fn(async () => null),
      },
    });

    await service.openExistingSave({
      cubeId: 'artist/cubes/SDXL/Existing.cube',
      defaultAlias: 'SDXL/Existing',
      targetModel: 'SDXL',
    });

    expect(openCubeAuthoring).toHaveBeenCalledWith(
      expect.objectContaining({
        destinationLocked: true,
        destinations: [
          expect.objectContaining({
            destination: {
              kind: 'pack',
              owner: 'artist',
              repo: 'cubes',
              repoRef: 'artist/cubes',
            },
          }),
        ],
      }),
    );
  });
});
