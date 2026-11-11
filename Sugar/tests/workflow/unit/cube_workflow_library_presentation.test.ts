//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify Cube editability follows source ownership rather than content equality. */

import { jest } from '@jest/globals';
import { CubeWorkflowLibraryPresentation } from '../../../frontend/comfyui/ui/workflow/CubeWorkflowLibraryPresentation.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubeWorkflowLibraryState } from '../../../frontend/comfyui/ui/workflow/CubeWorkflowLibraryState.js';

test('allows an owned divergent Cube to open in the editor', async () => {
  const canWriteCube = jest.fn(async () => true);
  const presentation = new CubeWorkflowLibraryPresentation(libraryState('read_only'), {
    canWriteCube,
  });
  const node = cubeNode('Artificial-Sweetener/Base-Cubes/Anima/Prompt by Region.cube');

  await expect(presentation.canEdit(node)).resolves.toBe(true);
  expect(canWriteCube).toHaveBeenCalledWith(
    'Artificial-Sweetener/Base-Cubes/Anima/Prompt by Region.cube',
  );
});

test('keeps an unowned Cube read-only independently of content divergence', async () => {
  const canWriteCube = jest.fn(async () => false);
  const presentation = new CubeWorkflowLibraryPresentation(libraryState('read_only'), {
    canWriteCube,
  });

  await expect(presentation.canEdit(cubeNode('someone/repo/cube.cube'))).resolves.toBe(false);
});

test('trusts writable classification without another catalog request', async () => {
  const canWriteCube = jest.fn(async () => false);
  const presentation = new CubeWorkflowLibraryPresentation(libraryState('writable'), {
    canWriteCube,
  });

  await expect(presentation.canEdit(cubeNode('owner/repo/cube.cube'))).resolves.toBe(true);
  expect(canWriteCube).not.toHaveBeenCalled();
});

/** Provide one resolved classification without coupling tests to its payload parser. */
function libraryState(access: 'read_only' | 'writable'): CubeWorkflowLibraryState {
  return {
    read: () => ({ access }),
  } as unknown as CubeWorkflowLibraryState;
}

/** Build the minimal native Cube contract required by edit authorization. */
function cubeNode(cubeId: string): CubeNode {
  return {
    id: 'cube-node',
    type: 'cube-definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-instance', cube_id: cubeId },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
