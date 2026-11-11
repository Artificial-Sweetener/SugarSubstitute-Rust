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
import { expect, jest, test } from '@jest/globals';
import { CubeVersionSwitchService } from '../../../frontend/comfyui/ui/cube/version/CubeVersionSwitchService.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { ConstructedCube } from '../../../frontend/comfyui/ui/cube/CubeConstructionService.js';

test('loads exact source and target artifacts before preserving instance geometry and replacing', async () => {
  const source = cube('1.0.0');
  source.pos[0] = 125;
  source.pos[1] = 250;
  source.size[0] = 735;
  source.size[1] = 415;
  const target = cube('2.0.0');
  const options = [
    { label: 'v2.0.0', value: '2.0.0', revisionRef: 'WORKTREE', current: true, raw: null },
    { label: 'v1.0.0', value: '1.0.0', revisionRef: 'abc', current: false, raw: null },
  ];
  const payload = { cube: { cube_id: 'cube.test' }, boundaries: { inputs: [], outputs: [] } };
  const loadArtifact = jest.fn(async () => payload);
  const constructed: ConstructedCube = {
    node: target,
    subgraph: target.subgraph,
    warnings: [],
    internalNodeCount: 0,
  };
  const construct = jest.fn(() => constructed);
  const replace = jest.fn();
  const service = new CubeVersionSwitchService({
    availability: { list: async () => options },
    repository: { loadArtifact },
    construction: { construct, discard: jest.fn() },
    replacement: { replace },
    definitions: { stage: (value) => ({ payload: value, definitionIds: [] }), discard: jest.fn() },
  });

  await expect(service.switch(source, '2.0.0')).resolves.toBe(target);
  expect(loadArtifact).toHaveBeenCalledTimes(2);
  expect(construct).toHaveBeenCalledWith(payload, {
    instanceId: 'instance',
    instanceAlias: 'My Demo',
    position: [125, 250],
    revisionRef: 'WORKTREE',
    size: [735, 415],
  });
  expect(replace).toHaveBeenCalledWith(expect.objectContaining({ source, target }));
});

function cube(version: string): CubeNode {
  const subgraph = {
    id: `definition-${version}`,
    name: 'Demo',
    _nodes: [],
    inputs: [],
    outputs: [],
  } as unknown as CubeNode['subgraph'];
  return {
    id: 'instance',
    title: 'My Demo',
    pos: [0, 0],
    size: [400, 300],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'instance',
        instance_alias: 'My Demo',
        cube_id: 'cube.test',
        cube_version: version,
      },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect: jest.fn(),
  };
}
