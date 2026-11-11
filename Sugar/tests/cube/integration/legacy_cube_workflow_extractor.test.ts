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
/** Characterize lossless removal of persisted marker-era Cube instances. */

import { LegacyCubeWorkflowExtractor } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeWorkflowExtractor.js';

describe('LegacyCubeWorkflowExtractor', () => {
  test('extracts a managed group without allowing its nodes or markers to configure', () => {
    const workflow = {
      nodes: [
        node(1, 'Source', [0, 0], 'source'),
        marker(10, 'SugarCubes.CubeInput', [100, 100], 'input.image', 'IMAGE'),
        node(11, 'Processor', [300, 150], 'body'),
        marker(12, 'SugarCubes.CubeOutput', [600, 100], 'output.image', 'IMAGE'),
        node(2, 'Sink', [900, 0], 'sink'),
      ],
      links: [
        [100, 1, 0, 10, 0, 'IMAGE'],
        [101, 10, 0, 11, 0, 'IMAGE'],
        [102, 11, 0, 12, 0, 'IMAGE'],
        [103, 12, 0, 2, 0, 'IMAGE'],
        [104, 1, 0, 2, 1, 'IMAGE'],
      ],
      groups: [
        {
          id: 5,
          title: 'Legacy Cube',
          bounding: [80, 80, 700, 500],
          sugarcubes: {
            schema: 5,
            managed: true,
            instance_id: 'instance-a',
            cube_id: 'cube-a',
            cube_name: 'Legacy Cube',
            alias: 'Placed Cube',
            nodes: ['11'],
            markers: { inputs: ['10'], outputs: ['12'] },
          },
        },
        { id: 6, title: 'Inner layout', bounding: [250, 120, 300, 250] },
        { id: 7, title: 'Unrelated', bounding: [1200, 0, 300, 300] },
      ],
    };

    const batch = new LegacyCubeWorkflowExtractor().extractInPlace(workflow);

    expect(batch.plans).toHaveLength(1);
    expect(batch.plans[0]).toMatchObject({
      key: 'instance-a',
      cubeId: 'cube-a',
      title: 'Placed Cube',
      position: [80, 80],
      size: [700, 500],
    });
    expect(batch.plans[0]?.nodes.map((entry) => entry.id)).toEqual([11]);
    expect(batch.plans[0]?.groups.map((entry) => entry.id)).toEqual([6]);
    expect(batch.plans[0]?.inputs).toEqual([
      {
        markerId: 10,
        name: 'input.image',
        type: 'IMAGE',
        targets: [{ linkId: 101, nodeId: 11, slot: 0 }],
      },
    ]);
    expect(batch.plans[0]?.outputs).toEqual([
      {
        markerId: 12,
        name: 'output.image',
        type: 'IMAGE',
        source: { linkId: 102, nodeId: 11, slot: 0 },
      },
    ]);
    expect(batch.plans[0]?.internalLinks).toEqual([]);
    expect(batch.connections).toEqual([
      {
        origin: { kind: 'root', nodeId: 1, slot: 0 },
        target: {
          kind: 'cube-input',
          cubeKey: 'instance-a',
          name: 'input.image',
        },
        type: 'IMAGE',
      },
      {
        origin: {
          kind: 'cube-output',
          cubeKey: 'instance-a',
          name: 'output.image',
        },
        target: { kind: 'root', nodeId: 2, slot: 0 },
        type: 'IMAGE',
      },
    ]);

    expect(workflow.nodes.map((entry) => entry.id)).toEqual([1, 2]);
    expect(workflow.links).toEqual([[104, 1, 0, 2, 1, 'IMAGE']]);
    expect(workflow.groups.map((entry) => entry.id)).toEqual([7]);
  });

  test('represents a persisted Cube-to-Cube link once through semantic boundaries', () => {
    const workflow = {
      nodes: [
        node(11, 'Producer', [100, 100], 'producer'),
        marker(12, 'SugarCubes.CubeOutput', [300, 100], 'output.image', 'IMAGE'),
        marker(20, 'SugarCubes.CubeInput', [500, 100], 'input.image', 'IMAGE'),
        node(21, 'Consumer', [700, 100], 'consumer'),
      ],
      links: [
        [1, 11, 0, 12, 0, 'IMAGE'],
        [2, 12, 0, 20, 0, 'IMAGE'],
        [3, 20, 0, 21, 0, 'IMAGE'],
      ],
      groups: [
        managedGroup(1, 'cube-one', [80, 80, 300, 300], ['11'], [], ['12']),
        managedGroup(2, 'cube-two', [480, 80, 300, 300], ['21'], ['20'], []),
      ],
    };

    const batch = new LegacyCubeWorkflowExtractor().extractInPlace(workflow);

    expect(batch.plans).toHaveLength(2);
    expect(batch.connections).toEqual([
      {
        origin: {
          kind: 'cube-output',
          cubeKey: 'cube-one',
          name: 'output.image',
        },
        target: {
          kind: 'cube-input',
          cubeKey: 'cube-two',
          name: 'input.image',
        },
        type: 'IMAGE',
      },
    ]);
    expect(workflow.nodes).toEqual([]);
    expect(workflow.links).toEqual([]);
    expect(workflow.groups).toEqual([]);
  });

  test('preserves structured legacy Cube identity when extracting a managed group', () => {
    const workflow = {
      nodes: [node(11, 'Processor', [100, 100], 'processor')],
      links: [],
      groups: [
        {
          id: 5,
          title: 'Ignored group title',
          bounding: [50, 80, 700, 500],
          sugarcubes: {
            managed: true,
            definition: {
              cube_id: 'local/personal/Detailer.cube',
              cube_version: '1.2.3',
              default_alias: 'Detailer',
            },
            instance: {
              instance_id: 'instance-detailer',
              instance_alias: 'My Detailer',
              nodes: ['11'],
              markers: { inputs: [], outputs: [] },
            },
            surface_state: { schema: 1 },
          },
        },
      ],
    };

    const batch = new LegacyCubeWorkflowExtractor().extractInPlace(workflow);

    expect(batch.plans).toEqual([
      expect.objectContaining({
        key: 'instance-detailer',
        cubeId: 'local/personal/Detailer.cube',
        cubeVersion: '1.2.3',
        title: 'My Detailer',
        metadata: expect.objectContaining({ surface_state: { schema: 1 } }),
      }),
    ]);
    expect(workflow.nodes).toEqual([]);
    expect(workflow.groups).toEqual([]);
  });
});

function node(id: number, type: string, pos: [number, number], symbol: string) {
  return {
    id,
    type,
    pos,
    size: [200, 100],
    flags: {},
    order: id,
    mode: 0,
    inputs: [{ name: 'value', type: 'IMAGE', link: null }],
    outputs: [{ name: 'value', type: 'IMAGE', links: null }],
    properties: { sugarcubes_symbol: symbol },
    widgets_values: [],
  };
}

function marker(id: number, type: string, pos: [number, number], symbol: string, slotType: string) {
  return {
    ...node(id, type, pos, symbol),
    inputs: [{ name: 'value', type: slotType, link: null }],
    outputs: [{ name: 'value', type: slotType, links: null }],
  };
}

function managedGroup(
  id: number,
  instanceId: string,
  bounding: [number, number, number, number],
  nodes: string[],
  inputs: string[],
  outputs: string[],
) {
  return {
    id,
    title: instanceId,
    bounding,
    sugarcubes: {
      schema: 5,
      managed: true,
      instance_id: instanceId,
      cube_id: instanceId,
      alias: instanceId,
      nodes,
      markers: { inputs, outputs },
    },
  };
}
