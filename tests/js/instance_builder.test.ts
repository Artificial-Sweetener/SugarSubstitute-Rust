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
import { InstanceBuilder } from '../../web/comfyui/ui/graph/InstanceBuilder.js';
import type {
  ComfyGraph,
  ComfyInput,
  ComfyLink,
  ComfyNode,
  ComfyOutput,
  ComfyWidget,
} from '../../web/comfyui/ui/types/graph.js';
import type { UnknownRecord } from '../../web/comfyui/ui/types/common.js';

interface TestNode extends ComfyNode {
  id: number;
  type: string;
  pos: number[];
  size: number[];
  inputs: ComfyInput[];
  outputs: ComfyOutput[];
  widgets: ComfyWidget[];
  graph: TestGraph;
}

interface TestNodeList extends Array<TestNode> {
  find(predicate: (value: TestNode, index: number, obj: TestNode[]) => unknown): TestNode;
}

interface TestGraph extends ComfyGraph {
  _nodes: TestNodeList;
  links: Record<string, ComfyLink>;
  _links?: Map<string | number, ComfyLink>;
}

function outputLinks(node: TestNode): Array<string | number> {
  const output = node.outputs[0];
  if (!output) throw new Error(`Missing output on ${node.type}`);
  output.links ??= [];
  return output.links;
}

function widget(node: TestNode, name: string): ComfyWidget {
  const found = node.widgets.find((entry) => entry.name === name);
  if (!found) throw new Error(`Missing ${name} widget on ${node.type}`);
  return found;
}

function makeGraph(): TestGraph {
  const graph = { _nodes: [] as unknown as TestNodeList, links: {} } as TestGraph;
  const sampler: TestNode = {
    id: 1,
    type: 'KSampler',
    pos: [0, 0],
    size: [100, 50],
    inputs: [],
    outputs: [{ type: 'IMAGE', links: [] }],
    widgets: [],
    graph,
  };
  const outputMarker: TestNode = {
    id: 2,
    type: 'SugarCubes.CubeOutput',
    pos: [120, 0],
    size: [80, 40],
    inputs: [{ type: 'IMAGE', link: 1 }],
    outputs: [{ type: 'IMAGE', links: [] }],
    widgets: [
      { name: 'cube_id', value: 'local/example-user/out.cube' },
      { name: 'default_alias', value: 'Demo Cube' },
      { name: 'instance_id', value: '' },
    ],
    graph,
  };
  const inputMarker: TestNode = {
    id: 3,
    type: 'SugarCubes.CubeInput',
    pos: [240, 0],
    size: [80, 40],
    inputs: [{ type: 'IMAGE', link: null }],
    outputs: [{ type: 'IMAGE', links: [] }],
    widgets: [
      { name: 'cube_id', value: 'local/example-user/out.cube' },
      { name: 'default_alias', value: 'Demo Cube' },
      { name: 'instance_id', value: '' },
    ],
    graph,
  };

  graph.links[1] = {
    id: 1,
    origin_id: sampler.id,
    origin_slot: 0,
    target_id: outputMarker.id,
    target_slot: 0,
    type: 'IMAGE',
  };
  outputLinks(sampler).push(1);
  graph._nodes.push(sampler, outputMarker, inputMarker);
  return graph;
}

describe('InstanceBuilder', () => {
  test('builds instances from marker groups', () => {
    const graph = makeGraph();
    const builder = new InstanceBuilder();
    const instances = builder.build(graph);

    expect(instances).toHaveLength(2);
    const withNodes = instances.find((entry) => entry.nodeIds.includes('1'));
    expect(withNodes).toBeTruthy();
    expect(withNodes!.cubeId).toBe('local/example-user/out.cube');
    expect(withNodes!.defaultAlias).toBe('Demo Cube');
  });

  test('builds instances when links are only present on node slots', () => {
    const graph = makeGraph();
    graph.links = new Map() as unknown as Record<string, ComfyLink>;
    graph._links = new Map();
    const builder = new InstanceBuilder();
    const instances = builder.build(graph);

    expect(instances).toHaveLength(2);
    const withNodes = instances.find((entry) => entry.nodeIds.includes('1'));
    expect(withNodes).toBeTruthy();
    expect(withNodes!.cubeId).toBe('local/example-user/out.cube');
  });

  test('ignores marker candidates when cube_id is missing', () => {
    const graph = makeGraph();
    const inputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeInput');
    const sampler = graph._nodes.find((node) => node.type === 'KSampler');
    graph.links[2] = {
      id: 2,
      origin_id: inputMarker.id,
      origin_slot: 0,
      target_id: sampler.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    sampler.inputs = [{ type: 'IMAGE', link: 2 }];
    outputLinks(inputMarker).push(2);
    graph._nodes.forEach((node) => {
      if (!node.widgets) {
        return;
      }
      const cubeId = node.widgets.find((widget) => widget.name === 'cube_id');
      if (cubeId) {
        cubeId.value = '';
      }
    });
    let counter = 0;
    const builder = new InstanceBuilder({
      instanceIdFactory: () => `inst-${(counter += 1)}`,
    });
    const instances = builder.build(graph);

    expect(instances).toEqual([]);
  });

  test('uses marker instance_id when present', () => {
    const graph = makeGraph();
    graph._nodes.forEach((node) => {
      if (!node.widgets) {
        return;
      }
      const instanceId = node.widgets.find((widget) => widget.name === 'instance_id');
      if (instanceId) {
        instanceId.value = 'inst-123';
      }
    });
    const builder = new InstanceBuilder({
      instanceIdFactory: () => 'inst-factory',
    });
    const instances = builder.build(graph);
    expect(instances[0].instanceId).toBe('inst-123');
  });

  test('generates instance_id when markers are missing it', () => {
    const graph = makeGraph();
    const builder = new InstanceBuilder({
      instanceIdFactory: () => 'inst-generated',
    });
    const instances = builder.build(graph);
    expect(instances[0].instanceId).toBe('inst-generated');
  });

  test('uses instance_id instead of default_alias for identity', () => {
    const graph = makeGraph();
    const outputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeOutput');
    const inputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeInput');
    const sampler = graph._nodes.find((node) => node.type === 'KSampler');
    graph.links[2] = {
      id: 2,
      origin_id: inputMarker.id,
      origin_slot: 0,
      target_id: sampler.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    sampler.inputs = [{ type: 'IMAGE', link: 2 }];
    outputLinks(inputMarker).push(2);
    widget(outputMarker, 'instance_id').value = 'inst-shared';
    widget(inputMarker, 'instance_id').value = 'inst-shared';
    widget(outputMarker, 'default_alias').value = 'Alpha';
    widget(inputMarker, 'default_alias').value = 'Beta';
    const builder = new InstanceBuilder({
      instanceIdFactory: () => 'inst-fallback',
    });
    const instances = builder.build(graph);
    expect(instances).toHaveLength(1);
    expect(instances[0].instanceId).toBe('inst-shared');
  });

  test('keeps same-cube markers with different versions in separate instances', () => {
    const graph = makeGraph();
    const outputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeOutput');
    const inputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeInput');
    const sampler = graph._nodes.find((node) => node.type === 'KSampler');
    graph.links[2] = {
      id: 2,
      origin_id: inputMarker.id,
      origin_slot: 0,
      target_id: sampler.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    sampler.inputs = [{ type: 'IMAGE', link: 2 }];
    outputLinks(inputMarker).push(2);
    widget(outputMarker, 'instance_id').value = 'inst-shared';
    widget(inputMarker, 'instance_id').value = 'inst-shared';
    outputMarker.properties = { sugarcubes_cube_version: '1.0.0' };
    inputMarker.properties = { sugarcubes_cube_version: '1.2.0' };

    const builder = new InstanceBuilder({
      instanceIdFactory: () => 'inst-fallback',
    });
    const instances = builder.build(graph);

    expect(instances).toHaveLength(2);
    expect(instances.map((entry) => entry.cubeVersion).sort()).toEqual(['1.0.0', '1.2.0']);
    expect(instances.map((entry) => entry.cubeDefinitionKey).sort()).toEqual([
      'local/example-user/out.cube@1.0.0',
      'local/example-user/out.cube@1.2.0',
    ]);
  });

  test('separates markers with different instance_id values into distinct instances', () => {
    const graph = makeGraph();
    const outputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeOutput');
    const inputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeInput');
    const sampler = graph._nodes.find((node) => node.type === 'KSampler');
    graph.links[2] = {
      id: 2,
      origin_id: inputMarker.id,
      origin_slot: 0,
      target_id: sampler.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    sampler.inputs = [{ type: 'IMAGE', link: 2 }];
    outputLinks(inputMarker).push(2);
    widget(outputMarker, 'instance_id').value = 'b';
    widget(inputMarker, 'instance_id').value = 'a';
    const warnings: Array<{ message: string; payload: unknown }> = [];
    const builder = new InstanceBuilder({
      logger: { warn: (message: string, payload: unknown) => warnings.push({ message, payload }) },
      instanceIdFactory: () => 'inst-generated',
    });
    const instances = builder.build(graph);
    const ids = instances.map((entry) => entry.instanceId).sort();
    expect(ids).toEqual(['a', 'b']);
    expect(warnings).toHaveLength(0);
  });

  test('keeps default_alias per component when cube_id is shared', () => {
    const graph = makeGraph();
    const outputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeOutput');
    const inputMarker = graph._nodes.find((node) => node.type === 'SugarCubes.CubeInput');
    widget(inputMarker, 'default_alias').value = 'Second Cube';
    const samplerA = graph._nodes.find((node) => node.type === 'KSampler');
    const samplerB: TestNode = {
      id: 4,
      type: 'KSampler',
      pos: [400, 0],
      size: [100, 50],
      inputs: [],
      outputs: [{ type: 'IMAGE', links: [] }],
      widgets: [],
      graph,
    };
    graph.links[2] = {
      id: 2,
      origin_id: inputMarker.id,
      origin_slot: 0,
      target_id: samplerB.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    samplerB.inputs = [{ type: 'IMAGE', link: 2 }];
    outputLinks(inputMarker).push(2);
    graph._nodes.push(samplerB);
    const builder = new InstanceBuilder({ instanceIdFactory: () => 'inst-generated' });
    const instances = builder.build(graph);

    expect(instances).toHaveLength(2);
    const names = instances.map((entry) => entry.defaultAlias).sort();
    expect(names).toEqual(['Demo Cube', 'Second Cube']);
    expect(widget(outputMarker, 'default_alias').value).toBe('Demo Cube');
    expect(widget(inputMarker, 'default_alias').value).toBe('Second Cube');
  });

  test('keeps same-cube instances separate when marker instance_id differs', () => {
    const sharedCubeId = 'local/shared';
    const graph = { _nodes: [] as unknown as TestNodeList, links: {} } as TestGraph;

    const nodeA: TestNode = {
      id: 1,
      type: 'KSampler',
      pos: [0, 0],
      size: [120, 60],
      inputs: [{ type: 'IMAGE', link: 101 }],
      outputs: [{ type: 'IMAGE', links: [102] }],
      widgets: [],
      graph,
    };
    const inputA: TestNode = {
      id: 2,
      type: 'SugarCubes.CubeInput',
      pos: [-120, 0],
      size: [80, 40],
      inputs: [{ type: 'IMAGE', link: null }],
      outputs: [{ type: 'IMAGE', links: [101] }],
      widgets: [
        { name: 'cube_id', value: sharedCubeId },
        { name: 'default_alias', value: 'Shared Cube' },
        { name: 'instance_alias', value: 'Shared Cube' },
        { name: 'instance_id', value: 'inst-a' },
      ],
      graph,
    };
    const outputA: TestNode = {
      id: 3,
      type: 'SugarCubes.CubeOutput',
      pos: [140, 0],
      size: [80, 40],
      inputs: [{ type: 'IMAGE', link: 102 }],
      outputs: [{ type: 'IMAGE', links: [103] }],
      widgets: [
        { name: 'cube_id', value: sharedCubeId },
        { name: 'default_alias', value: 'Shared Cube' },
        { name: 'instance_alias', value: 'Shared Cube' },
        { name: 'instance_id', value: 'inst-a' },
      ],
      graph,
    };

    const nodeB: TestNode = {
      id: 4,
      type: 'KSampler',
      pos: [360, 0],
      size: [120, 60],
      inputs: [{ type: 'IMAGE', link: 201 }],
      outputs: [{ type: 'IMAGE', links: [202] }],
      widgets: [],
      graph,
    };
    const inputB: TestNode = {
      id: 5,
      type: 'SugarCubes.CubeInput',
      pos: [240, 0],
      size: [80, 40],
      inputs: [{ type: 'IMAGE', link: 103 }],
      outputs: [{ type: 'IMAGE', links: [201] }],
      widgets: [
        { name: 'cube_id', value: sharedCubeId },
        { name: 'default_alias', value: 'Shared Cube' },
        { name: 'instance_alias', value: 'Shared Cube' },
        { name: 'instance_id', value: 'inst-b' },
      ],
      graph,
    };
    const outputB: TestNode = {
      id: 6,
      type: 'SugarCubes.CubeOutput',
      pos: [500, 0],
      size: [80, 40],
      inputs: [{ type: 'IMAGE', link: 202 }],
      outputs: [{ type: 'IMAGE', links: [] }],
      widgets: [
        { name: 'cube_id', value: sharedCubeId },
        { name: 'default_alias', value: 'Shared Cube' },
        { name: 'instance_alias', value: 'Shared Cube' },
        { name: 'instance_id', value: 'inst-b' },
      ],
      graph,
    };

    graph.links[101] = {
      id: 101,
      origin_id: inputA.id,
      origin_slot: 0,
      target_id: nodeA.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    graph.links[102] = {
      id: 102,
      origin_id: nodeA.id,
      origin_slot: 0,
      target_id: outputA.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    graph.links[103] = {
      id: 103,
      origin_id: outputA.id,
      origin_slot: 0,
      target_id: inputB.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    graph.links[201] = {
      id: 201,
      origin_id: inputB.id,
      origin_slot: 0,
      target_id: nodeB.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    graph.links[202] = {
      id: 202,
      origin_id: nodeB.id,
      origin_slot: 0,
      target_id: outputB.id,
      target_slot: 0,
      type: 'IMAGE',
    };

    graph._nodes.push(nodeA, inputA, outputA, nodeB, inputB, outputB);

    const builder = new InstanceBuilder();
    const instances = builder.build(graph);
    const instanceIds = instances.map((entry) => entry.instanceId).sort();

    expect(instanceIds).toEqual(['inst-a', 'inst-b']);
  });
});
