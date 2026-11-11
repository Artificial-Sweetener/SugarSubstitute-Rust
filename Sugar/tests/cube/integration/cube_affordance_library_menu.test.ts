//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify first-class Cube menus honor workflow-library access policy. */

import { jest } from '@jest/globals';
import { CubeAffordanceHostIntegration } from '../../../frontend/comfyui/ui/affordance/CubeAffordanceHostIntegration.js';
import type { CubeHostAffordanceController } from '../../../frontend/comfyui/ui/affordance/CubeHostAffordanceController.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubeWorkflowLibraryActions } from '../../../frontend/comfyui/ui/workflow/CubeWorkflowLibraryActions.js';

test('read-only native Cube menu offers library operations instead of definition save', () => {
  const keep = jest.fn();
  const capture = jest.fn(async () => undefined);
  const syncSource = jest.fn(async () => undefined);
  const forkToLocal = jest.fn(async () => undefined);
  const libraryActions = {
    classification: () => ({
      definitionId: 'definition',
      cubeId: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
      cubeVersion: '1.0.0',
      semanticHash: 'a'.repeat(64),
      instanceIds: ['cube-1'],
      primaryClass: 'none',
      access: 'read_only',
      sourceAvailable: true,
      permittedOperations: new Set(['keep', 'capture', 'track_source', 'fork']),
    }),
    keep,
    capture,
    syncSource,
    forkToLocal,
  } as unknown as CubeWorkflowLibraryActions;
  const saveCube = jest.fn(async () => undefined);
  const integration = new CubeAffordanceHostIntegration({
    document,
    canvas: {
      selectedItems: new Set(),
      getNodeMenuOptions: () => [],
      getCanvasMenuOptions: () => [],
    },
    controller: { saveCube } as unknown as CubeHostAffordanceController,
    logger: console,
    libraryActions,
  });

  const items = integration.getNodeMenuItems(cubeNode());

  expect(items.map((item) => item.content)).toEqual([
    'Keep workflow copy',
    'Capture Cube',
    'Synchronize home source…',
    'Fork to Local Cubes…',
  ]);
  items.forEach((item) => item.callback());
  expect(saveCube).not.toHaveBeenCalled();
  expect(keep).toHaveBeenCalledWith('cube-1');
  expect(capture).toHaveBeenCalledWith('cube-1');
  expect(syncSource).toHaveBeenCalledWith('cube-1');
  expect(forkToLocal).toHaveBeenCalledWith('cube-1');
});

/** Build the minimum real first-class Cube shape used by host menu recognition. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Text to Image',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Text to Image',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
