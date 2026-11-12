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
/** Verify native instance rename never takes ownership of a Cube definition name. */

import { jest } from '@jest/globals';
import { CubeInstanceRenameGuard } from '../../../frontend/comfyui/ui/affordance/CubeInstanceRenameGuard.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';

test.each([
  ['Nodes 1.0', false],
  ['Nodes 2.0', true],
] as const)('retains only the local title change in %s', async (_renderer, usesHeader) => {
  document.body.replaceChildren();
  const cube = cubeNode();
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  const changed = jest.fn();
  catalog.subscribe(changed);
  const canvas = { selectedItems: new Set<unknown>([cube]) };
  const guard = new CubeInstanceRenameGuard({
    document,
    canvas,
    nodes: catalog,
    contexts: new CubeEditorContextResolver(catalog),
  });
  guard.install();
  const input = document.createElement('input');
  input.setAttribute('data-testid', 'node-title-input');
  if (usesHeader) {
    const header = document.createElement('div');
    header.setAttribute('data-testid', 'node-header-cube-node');
    header.append(input);
    document.body.append(header);
  } else {
    document.body.append(input);
  }
  input.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));

  cube.title = 'Local placement';
  cube.subgraph.name = 'Local placement';
  input.addEventListener('keydown', () => {
    queueMicrotask(() => {
      cube.subgraph.name = 'Deferred host definition name';
    });
  });
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await new Promise((resolve) => window.setTimeout(resolve, 0));

  expect(cube.title).toBe('Local placement');
  expect(cube.subgraph.name).toBe('Cube: Shared definition');
  expect(changed).toHaveBeenCalledTimes(1);
  guard.dispose();
});

/** Build one marked Cube with deliberately different instance and definition names. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-node',
    type: 'definition',
    title: 'Placement',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-instance' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Cube: Shared definition',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
