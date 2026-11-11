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
/** Verify renderer-independent Cube affordance decisions. */

import { CubeAffordancePolicy } from '../../../frontend/comfyui/ui/affordance/CubeAffordancePolicy.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubeSelectionContext } from '../../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';

const policy = new CubeAffordancePolicy();

test('blocks structural operations and maps persistence for a Cube selection', () => {
  const selection = cubeSelection('cube_draft');
  expect(policy.decide('unpack', selection, null)).toEqual(
    expect.objectContaining({ visible: false, enabled: false, intent: 'blocked' }),
  );
  expect(policy.decide('convert', selection, null)).toEqual(
    expect.objectContaining({ visible: false, enabled: false, intent: 'blocked' }),
  );
  expect(policy.decide('publish', selection, null)).toEqual(
    expect.objectContaining({ label: 'Save Cube', intent: 'save-cube', targetKind: 'draft-cube' }),
  );
  expect(policy.decide('configure-interface', selection, null)).toEqual(
    expect.objectContaining({ visible: false, enabled: false, intent: 'blocked' }),
  );
  const mixedSelection = {
    ...selection,
    items: [...selection.items, { type: 'ordinary' }],
    isSingleCube: false,
  };
  expect(policy.decide('configure-interface', mixedSelection, null).visible).toBe(false);
});

test('preserves native actions for an ordinary Subgraph', () => {
  const ordinary = { isSubgraphNode: () => true, subgraph: {} };
  const selection: CubeSelectionContext = {
    items: [ordinary],
    cubeNodes: [],
    ordinarySubgraphNodes: [ordinary],
    containsCube: false,
    isSingleCube: false,
    isSingleOrdinarySubgraph: true,
  };
  expect(policy.decide('publish', selection, null).intent).toBe('native');
  expect(policy.decide('unpack', selection, null).intent).toBe('native');
  expect(policy.decide('configure-interface', selection, null).intent).toBe('native');
});

test('adapts only the Cube root while preserving a nested ordinary Subgraph editor', () => {
  const selection = emptySelection();
  const node = cubeNode('cube');
  const root = { node, path: [node.subgraph], isCubeRoot: true };
  const nested = { node, path: [node.subgraph, {}], isCubeRoot: false };
  expect(policy.decide('save-workflow', selection, root)).toEqual(
    expect.objectContaining({ label: 'Save Cube', intent: 'save-cube-editor' }),
  );
  expect(policy.decide('set-search-aliases', selection, root).visible).toBe(false);
  expect(policy.decide('configure-interface', selection, root).visible).toBe(false);
  expect(policy.decide('exit-container', selection, nested).intent).toBe('native');
});

/** Build a complete empty selection context. */
function emptySelection(): CubeSelectionContext {
  return {
    items: [],
    cubeNodes: [],
    ordinarySubgraphNodes: [],
    containsCube: false,
    isSingleCube: false,
    isSingleOrdinarySubgraph: false,
  };
}

/** Build a single marked Cube selection. */
function cubeSelection(kind: 'cube' | 'cube_draft'): CubeSelectionContext {
  const node = cubeNode(kind);
  return {
    items: [node],
    cubeNodes: [node],
    ordinarySubgraphNodes: [],
    containsCube: true,
    isSingleCube: true,
    isSingleOrdinarySubgraph: false,
  };
}

/** Build the structural Cube surface needed by the policy. */
function cubeNode(kind: 'cube' | 'cube_draft'): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: kind,
      sugarcubes_cube: { instance_id: 'cube-1', cube_id: 'cube.cube' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
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
