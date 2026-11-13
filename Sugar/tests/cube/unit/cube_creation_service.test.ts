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
} from '../../../frontend/comfyui/ui/create/CubeCreationService.js';
import type {
  AuthoredCube,
  AuthoredCubeDraft,
} from '../../../frontend/comfyui/ui/cube/ComfyCubeAuthoringAdapter.js';
import type {
  CubeAuthoringCandidate,
  CubeAuthoringValues,
} from '../../../frontend/comfyui/ui/create/CubeAuthoringDialog.js';
import type { CubeSaveOutcome } from '../../../frontend/comfyui/ui/save/CubeSaveService.js';

const draft = { node: {}, subgraph: {}, identity: {} } as unknown as AuthoredCubeDraft;
const cube = { node: {}, subgraph: {}, identity: {} } as unknown as AuthoredCube;
const localValues: CubeAuthoringValues = {
  name: 'Detailer',
  defaultAlias: 'SDXL/Detailer',
  cubeId: 'local/personal/SDXL/Detailer.cube',
  targetModel: 'SDXL',
  supportedModels: ['SDXL'],
  description: 'Detail images.',
  destination: { kind: 'local' },
};

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

function authoringSession(values: CubeAuthoringValues | null = null) {
  return { openFirstSave: jest.fn(async (_candidate: CubeAuthoringCandidate) => values) };
}

async function emptySave(): Promise<CubeSaveOutcome> {
  return { status: 'no_changes', savedCubeIds: [] };
}

describe('CubeCreationService', () => {
  test('creates an empty draft without opening metadata or saving', async () => {
    const createEmptyDraft = jest.fn(() => draft);
    const authoring = authoringSession();
    const service = new CubeCreationService({
      authoring,
      getAuthoring: () => createAuthoring({ createEmptyDraft }),
      cubeSave: { save: emptySave },
      createInstanceId: () => 'draft-1',
    });

    await expect(service.startCreateEmptyCube()).resolves.toBe(draft);
    expect(createEmptyDraft).toHaveBeenCalledWith({
      instanceId: 'draft-1',
      defaultAlias: 'Untitled Cube',
    });
    expect(authoring.openFirstSave).not.toHaveBeenCalled();
  });

  test('extracts a selection as a draft without guessing metadata or saving', async () => {
    const validateSelection = jest.fn();
    const createDraftFromSelection = jest.fn(() => draft);
    const service = new CubeCreationService({
      authoring: authoringSession(),
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

  test('promotes a draft through the established save contract after the shared authoring session', async () => {
    const promoteDraft = jest.fn(() => cube);
    const save = jest.fn(async () => ({
      status: 'saved' as const,
      savedCubeIds: [localValues.cubeId],
    }));
    const authoring = authoringSession(localValues);
    const service = new CubeCreationService({
      authoring,
      getAuthoring: () => createAuthoring({ promoteDraft }),
      cubeSave: { save },
    });

    await expect(
      service.saveDraft('draft-3', {
        nodeIds: [],
        markerIds: [],
        inputCount: 0,
        outputCount: 0,
      }),
    ).resolves.toBe(cube);
    expect(authoring.openFirstSave).toHaveBeenCalledWith({
      defaultAlias: 'SugarCube',
      targetModel: 'SDXL',
      warnings: [],
      nodeIds: [],
      markerIds: [],
      inputCount: 0,
      outputCount: 0,
    });
    expect(promoteDraft).toHaveBeenCalledWith(
      'draft-3',
      expect.objectContaining({
        cubeId: localValues.cubeId,
        instanceId: 'draft-3',
        description: 'Detail images.',
      }),
    );
    expect(save).toHaveBeenCalledWith({ cubeIds: [localValues.cubeId] });
  });

  test('routes editor metadata and graph counts through the same first-save session', async () => {
    const promoteDraft = jest.fn(() => cube);
    const save = jest.fn(async () => ({
      status: 'saved' as const,
      savedCubeIds: [localValues.cubeId],
    }));
    const authoring = authoringSession(localValues);
    const service = new CubeCreationService({
      authoring,
      getAuthoring: () => createAuthoring({ promoteDraft }),
      cubeSave: { save },
    });

    await service.saveDraftFromEditor(
      'draft-editor',
      {
        defaultAlias: 'Detailer',
        targetModel: 'SDXL',
        supportedModels: ['SDXL'],
        description: 'Detail images.',
        destination: 'local',
      },
      {
        nodeIds: ['node-a', 'node-b'],
        markerIds: ['input-a', 'output-a'],
        inputCount: 1,
        outputCount: 1,
      },
    );

    expect(authoring.openFirstSave).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultAlias: 'Detailer',
        nodeIds: ['node-a', 'node-b'],
        inputCount: 1,
        outputCount: 1,
      }),
    );
    expect(save).toHaveBeenCalledWith({ cubeIds: [localValues.cubeId] });
  });
});
