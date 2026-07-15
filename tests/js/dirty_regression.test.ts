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
import { describe, expect, test } from '@jest/globals';
import { BaselineResolver } from '../../web/comfyui/ui/graph/BaselineResolver.js';
import { BaselineStore } from '../../web/comfyui/ui/graph/BaselineStore.js';
import { DirtyTracker } from '../../web/comfyui/ui/graph/DirtyTracker.js';
import { computeDefinitionHash } from '../../web/comfyui/ui/graph/DirtyHasher.js';
import type { CubeGroupMetadataRecord } from '../../web/comfyui/ui/graph/GroupMetadata.js';

function createTracker() {
  const baselineStore = new BaselineStore();
  const baselineResolver = new BaselineResolver({ baselineStore });
  const tracker = new DirtyTracker({ baselineStore, baselineResolver });
  return { tracker, baselineStore };
}

function makeGraph({ cubeId = 'local/example-user/demo.cube' } = {}) {
  const nodes = [
    {
      id: 1,
      type: 'A',
      pos: [0, 0],
      size: [100, 50],
      widgets: [{ name: 'seed', value: 1 }],
      properties: { seed: 1 },
    },
  ];
  const sugarcubes: CubeGroupMetadataRecord = {
    managed: true,
    instance_id: 'inst-1',
    cube_id: cubeId,
    nodes: ['1'],
    markers: { inputs: [] },
  };
  const group = {
    title: 'Cube Group',
    pos: [0, 0],
    size: [300, 200],
    properties: {
      sugarcubes,
    },
  };
  return { graph: { _nodes: nodes, links: [], _groups: [group] }, group, nodes };
}

describe('dirty regression harness', () => {
  test('save -> clean -> edit -> dirty reappears', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph();

    tracker.markLocalBaseline({ graph, cubeIds: ['local/example-user/demo.cube'] });
    tracker.markClean({ graph, cubeIds: ['local/example-user/demo.cube'] });
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(group.properties.sugarcubes.dirty).toBe(false);

    nodes[0].properties.seed = 2;
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(group.properties.sugarcubes.dirty).toBe(true);
  });

  test('import -> clean -> edit -> dirty reappears', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph({ cubeId: 'local/example-user/imported.cube' });

    tracker.markLocalBaseline({ graph, cubeIds: ['local/example-user/imported.cube'] });
    tracker.markClean({ graph, cubeIds: ['local/example-user/imported.cube'] });
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/imported.cube'] });

    expect(group.properties.sugarcubes.dirty).toBe(false);

    nodes[0].properties.seed = 2;
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/imported.cube'] });

    expect(group.properties.sugarcubes.dirty).toBe(true);
  });

  test('definition loads after graph ready keeps dirty stable', () => {
    const { tracker, baselineStore } = createTracker();
    const { graph } = makeGraph();
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    const definition = {
      nodes: [
        {
          symbol: 'node_a',
          class_type: 'A',
          inputs: { seed: 1 },
          layout: { pos: [0, 0], size: [100, 50] },
        },
      ],
      markers: [],
      connections: [],
    };
    const definitionHash = computeDefinitionHash(definition);
    baselineStore.setDefinition('local/example-user/demo.cube', {
      hash: definitionHash,
      status: 'ready',
    });

    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
    expect(graph._groups[0].properties.sugarcubes.dirty).toBe(false);
  });
});
