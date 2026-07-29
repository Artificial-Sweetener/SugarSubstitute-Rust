//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify graph-only authoring drafts and first-save promotion. */

import { jest } from '@jest/globals';
import {
  CubeCreationService,
  type CubeCreationAuthoring,
} from '../../frontend/comfyui/ui/create/CubeCreationService.js';
import type {
  AuthoredCube,
  AuthoredCubeDraft,
} from '../../frontend/comfyui/ui/cube/ComfyCubeAuthoringAdapter.js';
import type {
  CubeAuthoringDialogOptions,
  CubeAuthoringValues,
} from '../../frontend/comfyui/ui/create/CubeAuthoringDialog.js';
import type { CubeSaveOutcome } from '../../frontend/comfyui/ui/save/CubeSaveService.js';

const draft = { node: {}, subgraph: {}, identity: {} } as unknown as AuthoredCubeDraft;
const cube = { node: {}, subgraph: {}, identity: {} } as unknown as AuthoredCube;

function createAuthoring(overrides: Partial<CubeCreationAuthoring> = {}): CubeCreationAuthoring {
  return {
    selectedCount: () => 0,
    validateSelection: () => undefined,
    createEmptyDraft: () => draft,
    createDraftFromSelection: () => draft,
    validateSelectedSubgraph: () => undefined,
    createDraftFromSelectedSubgraph: () => draft,
    promoteDraft: () => cube,
    restoreDraft: () => undefined,
    ...overrides,
  };
}

async function emptySave(): Promise<CubeSaveOutcome> {
  return { status: 'no_changes', savedCubeIds: [] };
}

async function cancelledAuthoring(): Promise<CubeAuthoringValues | null> {
  return null;
}

describe('CubeCreationService', () => {
  test('creates an empty draft without opening metadata or saving', async () => {
    const createEmptyDraft = jest.fn(() => draft);
    const service = new CubeCreationService({
      getAuthoring: () => createAuthoring({ createEmptyDraft }),
      cubeSave: { save: emptySave },
      dialogs: { openCubeAuthoring: cancelledAuthoring },
      createInstanceId: () => 'draft-1',
    });

    await expect(service.startCreateEmptyCube()).resolves.toBe(draft);
    expect(createEmptyDraft).toHaveBeenCalledWith({
      instanceId: 'draft-1',
      defaultAlias: 'Untitled Cube',
    });
  });

  test('extracts a selection as a draft without guessing metadata or saving', async () => {
    const validateSelection = jest.fn();
    const createDraftFromSelection = jest.fn(() => draft);
    const service = new CubeCreationService({
      getAuthoring: () =>
        createAuthoring({ selectedCount: () => 2, validateSelection, createDraftFromSelection }),
      cubeSave: { save: emptySave },
      createInstanceId: () => 'draft-2',
    });

    await expect(service.startCreateCubeFromSelection()).resolves.toBe(draft);
    expect(validateSelection).toHaveBeenCalledTimes(1);
    expect(createDraftFromSelection).toHaveBeenCalledWith({
      instanceId: 'draft-2',
      defaultAlias: 'Untitled Cube',
    });
  });

  test('promotes a draft through the established local save contract on first save', async () => {
    const promoteDraft = jest.fn(() => cube);
    const save = jest.fn(async () => ({
      status: 'saved' as const,
      savedCubeIds: ['local/personal/SDXL/Detailer.cube'],
    }));
    const service = new CubeCreationService({
      getAuthoring: () => createAuthoring({ promoteDraft }),
      cubeSave: { save },
      dialogs: {
        openCubeAuthoring: jest.fn(async () => ({
          name: 'Detailer',
          defaultAlias: 'SDXL/Detailer',
          cubeId: 'local/personal/SDXL/Detailer.cube',
          targetModel: 'SDXL',
          supportedModels: ['SDXL'],
          description: 'Detail images.',
          destination: { kind: 'local' as const },
        })),
      },
    });

    await expect(service.saveDraft('draft-3')).resolves.toBe(cube);
    expect(promoteDraft).toHaveBeenCalledWith(
      'draft-3',
      expect.objectContaining({
        cubeId: 'local/personal/SDXL/Detailer.cube',
        instanceId: 'draft-3',
        description: 'Detail images.',
      }),
    );
    expect(save).toHaveBeenCalledWith({ cubeIds: ['local/personal/SDXL/Detailer.cube'] });
  });

  test('derives a direct author-pack identity when the first-save destination is a pack', async () => {
    const openCubeAuthoring = async (
      options: CubeAuthoringDialogOptions,
    ): Promise<CubeAuthoringValues> => {
      if (!options.deriveIdentity) throw new Error('Missing identity derivation.');
      const identity = await options.deriveIdentity('Shared', 'SDXL', {
        kind: 'pack',
        owner: '',
        repo: '',
        repoRef: '',
      });
      return {
        ...identity,
        targetModel: 'SDXL',
        supportedModels: ['SDXL'],
        description: '',
        destination: {
          kind: 'pack' as const,
          owner: 'artist',
          repo: 'cubes',
          repoRef: 'artist/cubes',
        },
      };
    };
    const promoteDraft = jest.fn(() => cube);
    const service = new CubeCreationService({
      getAuthoring: () => createAuthoring({ promoteDraft }),
      cubeSave: {
        save: jest.fn(async () => ({
          status: 'saved' as const,
          savedCubeIds: ['artist/cubes/SDXL/Shared.cube'],
        })),
      },
      dialogs: { openCubeAuthoring },
      packService: {
        chooseWritablePack: jest.fn(async () => ({
          owner: 'artist',
          repo: 'cubes',
          repoRef: 'artist/cubes',
        })),
      },
    });

    await service.saveDraft('draft-pack');
    expect(promoteDraft).toHaveBeenCalledWith(
      'draft-pack',
      expect.objectContaining({ cubeId: 'artist/cubes/SDXL/Shared.cube' }),
    );
  });

  test('routes editor metadata through the established first-save authoring flow', async () => {
    const promoteDraft = jest.fn(() => cube);
    const save = jest.fn(async () => ({
      status: 'saved' as const,
      savedCubeIds: ['local/personal/SDXL/Editor Cube.cube'],
    }));
    const service = new CubeCreationService({
      getAuthoring: () => createAuthoring({ promoteDraft }),
      cubeSave: { save },
      dialogs: {
        openCubeAuthoring: jest.fn(async () => ({
          name: 'Editor Cube',
          defaultAlias: 'SDXL/Editor Cube',
          cubeId: 'local/personal/SDXL/Editor Cube.cube',
          targetModel: 'SDXL',
          supportedModels: ['SDXL'],
          description: 'Saved from the editor HUD.',
          destination: { kind: 'local' as const },
        })),
      },
    });

    await service.saveDraftFromEditor('draft-editor', {
      defaultAlias: 'Editor Cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL'],
      description: 'Saved from the editor HUD.',
      destination: 'local',
    });

    expect(promoteDraft).toHaveBeenCalledWith(
      'draft-editor',
      expect.objectContaining({
        cubeId: 'local/personal/SDXL/Editor Cube.cube',
        description: 'Saved from the editor HUD.',
      }),
    );
    expect(save).toHaveBeenCalledWith({ cubeIds: ['local/personal/SDXL/Editor Cube.cube'] });
  });

  test('chooses a writable author pack once when the editor saves a draft', async () => {
    const chooseWritablePack = jest.fn(async () => ({
      owner: 'artist',
      repo: 'cubes',
      repoRef: 'artist/cubes',
    }));
    const promoteDraft = jest.fn(() => cube);
    const openCubeAuthoring = jest.fn(async (options: CubeAuthoringDialogOptions) => {
      const deriveIdentity = options.deriveIdentity;
      if (!deriveIdentity) throw new Error('Expected Cube identity derivation.');
      const identity = await deriveIdentity('Editor Cube', 'SDXL', {
        kind: 'pack',
        owner: '',
        repo: '',
        repoRef: '',
      });
      return {
        ...identity,
        targetModel: 'SDXL',
        supportedModels: ['SDXL'],
        description: '',
        destination: { kind: 'pack' as const, owner: '', repo: '', repoRef: '' },
      };
    });
    const service = new CubeCreationService({
      getAuthoring: () => createAuthoring({ promoteDraft }),
      cubeSave: {
        save: jest.fn(async () => ({
          status: 'saved' as const,
          savedCubeIds: ['artist/cubes/SDXL/Editor Cube.cube'],
        })),
      },
      dialogs: { openCubeAuthoring },
      packService: { chooseWritablePack },
    });

    await service.saveDraftFromEditor('draft-editor', {
      defaultAlias: 'Editor Cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL'],
      description: '',
      destination: 'pack',
    });

    expect(chooseWritablePack).toHaveBeenCalledTimes(1);
    expect(promoteDraft).toHaveBeenCalledWith(
      'draft-editor',
      expect.objectContaining({ cubeId: 'artist/cubes/SDXL/Editor Cube.cube' }),
    );
  });
});
