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
/** Verify generated Subgraph descriptions do not leak through Cube node tooltips. */

import { CubeNodeTooltipAdapter } from '../../../frontend/comfyui/ui/affordance/CubeNodeTooltipAdapter.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('uses literal Cube metadata and restores the host tooltip on disposal', () => {
  document.body.replaceChildren();
  const header = document.createElement('div');
  header.setAttribute('data-testid', 'node-header-cube-node');
  const title = document.createElement('div');
  title.setAttribute('data-testid', 'node-title');
  title.setAttribute('aria-describedby', 'node-tooltip');
  header.append(title);
  const tooltip = document.createElement('div');
  tooltip.id = 'node-tooltip';
  const text = document.createElement('span');
  text.className = 'p-tooltip-text';
  text.textContent = 'Subgraph node for Cube';
  tooltip.append(text);
  document.body.append(header, tooltip);
  const catalog = new CubeNodeCatalog();
  catalog.add(cubeNode());
  const adapter = new CubeNodeTooltipAdapter({ document, nodes: catalog });

  adapter.install();
  expect(text.textContent).toBe('Literal <Cube> description');
  expect(text.querySelector('cube')).toBeNull();
  adapter.dispose();
  expect(text.textContent).toBe('Subgraph node for Cube');
});

/** Build one Cube with markup-like tooltip metadata. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-node',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'cube-instance',
        default_alias: '<Cube>',
        description: 'Literal <Cube> description',
      },
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
