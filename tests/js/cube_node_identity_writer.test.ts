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
/** Verify graph-owned metadata updates from the Cube editor. */

import { jest } from '@jest/globals';
import { updateCubeNodeIdentityForIds } from '../../frontend/comfyui/ui/cube/node/CubeNodeIdentityWriter.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('persists editable metadata fields including cleared target model', () => {
  const node = cubeNode();
  const changed = jest.fn();
  const catalog = new CubeNodeCatalog();
  catalog.add(node);
  catalog.subscribe(changed);

  const updated = updateCubeNodeIdentityForIds(catalog, ['cube-instance'], {
    defaultAlias: 'Updated Cube',
    targetModel: '',
    supportedModels: ['SDXL', 'Flux'],
    description: 'Updated description.',
  });

  expect(updated).toBe(1);
  expect(node.title).toBe('Updated Cube');
  expect(node.properties.sugarcubes_cube).toEqual(
    expect.objectContaining({
      target_model: '',
      supported_models: ['SDXL', 'Flux'],
      description: 'Updated description.',
    }),
  );
  expect(node.subgraph.description).toBe('Updated description.');
  expect(node.subgraph.extra?.sugarcubes_kind).toBe('cube');
  expect(changed).toHaveBeenCalledTimes(1);
});

/** Build the smallest real Cube node persistence shape for the writer. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-instance',
    title: 'Original Cube',
    pos: [0, 0],
    size: [320, 180],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'cube-instance',
        cube_id: 'local/personal/SDXL/original.cube',
        default_alias: 'Original Cube',
        target_model: 'SDXL',
      },
    },
    inputs: [],
    outputs: [],
    subgraph: { extra: {} } as never,
    connect() {},
    isSubgraphNode: () => true,
    serialize: () => ({}),
  } as CubeNode;
}
