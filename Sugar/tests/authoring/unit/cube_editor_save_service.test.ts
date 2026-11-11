//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify Cube-editor saves retain the full modal confirmation flow. */

import { jest } from '@jest/globals';
import type {
  CubeAuthoringCandidate,
  CubeAuthoringValues,
} from '../../../frontend/comfyui/ui/create/CubeAuthoringDialog.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorSaveService } from '../../../frontend/comfyui/ui/save/CubeEditorSaveService.js';

const editorValues = {
  defaultAlias: 'SDXL/Edited',
  targetModel: 'SDXL',
  supportedModels: ['SDXL', 'Flux .1 D'],
  description: 'Edited from the Cube graph.',
  destination: 'local' as const,
};

describe('CubeEditorSaveService', () => {
  test('opens the complete modal before saving an existing owned Cube', async () => {
    const node = cubeNode('cube');
    const catalog = new CubeNodeCatalog();
    catalog.add(node);
    const save = jest.fn(async () => ({
      status: 'saved' as const,
      savedCubeIds: ['local/personal/SDXL/Existing.cube'],
    }));
    const openExistingSave = jest.fn(
      async (candidate: CubeAuthoringCandidate): Promise<CubeAuthoringValues> => {
        expect(candidate).toEqual(
          expect.objectContaining({
            cubeId: 'local/personal/SDXL/Existing.cube',
            description: 'Edited from the Cube graph.',
            supportedModels: ['SDXL', 'Flux .1 D'],
            nodeIds: [],
            inputCount: 0,
            outputCount: 0,
          }),
        );
        return {
          name: 'Edited',
          defaultAlias: 'SDXL/Edited',
          cubeId: 'local/personal/SDXL/Existing.cube',
          targetModel: 'SDXL',
          supportedModels: ['SDXL', 'Flux .1 D'],
          description: 'Confirmed in the save modal.',
          destination: { kind: 'local' },
        };
      },
    );
    const service = new CubeEditorSaveService({
      getCatalog: () => catalog,
      authoring: { openExistingSave },
      cubeCreation: { saveDraftFromEditor: jest.fn(async () => null) },
      cubeSave: { save },
    });

    await expect(service.save(node, editorValues)).resolves.toBe('saved');

    expect(openExistingSave).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith({
      cubeIds: ['local/personal/SDXL/Existing.cube'],
    });
    expect(node.properties.sugarcubes_cube).toEqual(
      expect.objectContaining({
        cube_id: 'local/personal/SDXL/Existing.cube',
        default_alias: 'SDXL/Edited',
        description: 'Confirmed in the save modal.',
        supported_models: ['SDXL', 'Flux .1 D'],
      }),
    );
  });

  test('does not mutate or save when the existing-Cube modal is dismissed', async () => {
    const node = cubeNode('cube');
    const catalog = new CubeNodeCatalog();
    catalog.add(node);
    const save = jest.fn(async () => ({ status: 'saved' as const, savedCubeIds: [] }));
    const before = JSON.parse(JSON.stringify(node.properties.sugarcubes_cube)) as unknown;
    const service = new CubeEditorSaveService({
      getCatalog: () => catalog,
      authoring: { openExistingSave: jest.fn(async () => null) },
      cubeCreation: { saveDraftFromEditor: jest.fn(async () => null) },
      cubeSave: { save },
    });

    await expect(service.save(node, editorValues)).resolves.toBe('cancelled');
    expect(node.properties.sugarcubes_cube).toEqual(before);
    expect(save).not.toHaveBeenCalled();
  });

  test('routes a workflow-only draft through first-save promotion', async () => {
    const node = cubeNode('cube_draft');
    const saveDraftFromEditor = jest.fn(async () => ({ node: {} }));
    const service = new CubeEditorSaveService({
      getCatalog: () => null,
      authoring: { openExistingSave: jest.fn(async () => null) },
      cubeCreation: { saveDraftFromEditor },
      cubeSave: {
        save: jest.fn(async () => ({ status: 'no_changes' as const, savedCubeIds: [] })),
      },
    });

    await expect(service.save(node, editorValues)).resolves.toBe('saved');
    expect(saveDraftFromEditor).toHaveBeenCalledWith('cube-instance', editorValues, {
      nodeIds: [],
      markerIds: [],
      inputCount: 0,
      outputCount: 0,
    });
  });
});

/** Build the exact native Cube shape required by the editor-save boundary. */
function cubeNode(kind: 'cube' | 'cube_draft'): CubeNode {
  return {
    id: 'cube-instance',
    title: 'Existing',
    type: 'cube-definition',
    pos: [0, 0],
    size: [720, 480],
    inputs: [],
    outputs: [],
    properties: {
      sugarcubes_kind: kind,
      sugarcubes_cube: {
        instance_id: 'cube-instance',
        cube_id: kind === 'cube' ? 'local/personal/SDXL/Existing.cube' : '',
        default_alias: 'SDXL/Existing',
        target_model: 'SDXL',
        supported_models: ['SDXL'],
        description: 'Before edit.',
      },
      sugarcubes_surface: {},
    },
    subgraph: {
      id: 'cube-definition',
      name: 'Existing',
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        throw new Error('not used');
      },
      addOutput() {
        throw new Error('not used');
      },
      configure() {},
    },
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
