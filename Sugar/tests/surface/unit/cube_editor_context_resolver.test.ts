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
/** Verify shared Cube editor and selection context resolution. */

import { CubeEditorContextResolver } from '../../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

test('distinguishes a Cube root from an ordinary nested Subgraph', () => {
  const nested = { id: 'nested', _nodes: [] };
  const cubeGraph = {
    id: 'cube-definition',
    name: 'Cube',
    _nodes: [{ isSubgraphNode: () => true, subgraph: nested }],
  } as unknown as NativeCubeSubgraph;
  const catalog = new CubeNodeCatalog();
  const cube = cubeNode(cubeGraph);
  catalog.add(cube);
  const resolver = new CubeEditorContextResolver(catalog);

  expect(resolver.resolveEditor(cubeGraph)).toEqual({
    node: cube,
    path: [cubeGraph],
    isCubeRoot: true,
  });
  expect(resolver.resolveEditor(nested)).toEqual({
    node: cube,
    path: [cubeGraph, nested],
    isCubeRoot: false,
  });
  expect(resolver.resolveEditor({ id: 'outside' })).toBeNull();
});

test('classifies marked Cubes separately from ordinary native Subgraphs', () => {
  const cubeGraph = {
    id: 'cube-definition',
    name: 'Cube',
    _nodes: [],
    inputs: [],
    outputs: [],
  } as unknown as NativeCubeSubgraph;
  const cube = cubeNode(cubeGraph);
  const ordinary = { isSubgraphNode: () => true, subgraph: { id: 'ordinary', _nodes: [] } };
  const catalog = new CubeNodeCatalog();
  const resolver = new CubeEditorContextResolver(catalog);

  expect(resolver.resolveSelection([cube])).toEqual(
    expect.objectContaining({
      containsCube: true,
      isSingleCube: true,
      isSingleOrdinarySubgraph: false,
    }),
  );
  expect(resolver.resolveSelection([ordinary])).toEqual(
    expect.objectContaining({
      containsCube: false,
      isSingleCube: false,
      isSingleOrdinarySubgraph: true,
    }),
  );
  expect(resolver.resolveSelection([cube, ordinary])).toEqual(
    expect.objectContaining({
      containsCube: true,
      isSingleCube: false,
      isSingleOrdinarySubgraph: false,
    }),
  );
});

/** Build one structural Cube recognized by the shared marker predicate. */
function cubeNode(subgraph: NativeCubeSubgraph): CubeNode {
  return {
    id: 'cube-1',
    type: subgraph.id,
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1', cube_id: 'cube.cube' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
