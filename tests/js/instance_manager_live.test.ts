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
import { describe, expect, test, jest } from '@jest/globals';
import { InstanceManager } from '../../web/comfyui/ui/graph/InstanceManager.js';
import { getGroupSugarcubes } from '../../web/comfyui/ui/graph/GroupMetadata.js';
import type { CubeBoundsPayload } from '../../web/comfyui/ui/graph/CubeBounds.js';
import type {
  ComfyGraph,
  ComfyGroup,
  ComfyInput,
  ComfyNode,
  ComfyOutput,
  ComfyWidget,
} from '../../web/comfyui/ui/types/graph.js';

interface TestInput extends ComfyInput {
  link: number | null;
}
interface TestOutput extends ComfyOutput {
  links: number[];
}
interface TestNode extends ComfyNode {
  id: number;
  type: string;
  pos: number[];
  size: number[];
  inputs: TestInput[];
  outputs: TestOutput[];
  widgets: ComfyWidget[];
  graph?: ComfyGraph;
}

interface MarkerOptions {
  id: number;
  type: string;
  cubeId: string;
  defaultAlias: string;
  instanceId: string;
  alias: string;
}

function makeMarker({
  id,
  type,
  cubeId,
  defaultAlias,
  instanceId,
  alias,
}: MarkerOptions): TestNode {
  return {
    id,
    type,
    pos: [id * 10, 0],
    size: [80, 40],
    inputs: [{ name: 'value', link: null }],
    outputs: [{ name: 'value', links: [] }],
    widgets: [
      { name: 'cube_id', value: cubeId },
      { name: 'default_alias', value: defaultAlias },
      { name: 'instance_id', value: instanceId },
      { name: 'instance_alias', value: alias },
    ],
  };
}

function makeInternalNode(id: number): TestNode {
  return {
    id,
    type: 'KSampler',
    pos: [id * 10, 0],
    size: [120, 60],
    inputs: [{ name: 'value', link: null }],
    outputs: [{ name: 'value', links: [] }],
    properties: { sugarcubes_symbol: 'sampler' },
    widgets: [],
  };
}

describe('InstanceManager metadata', () => {
  test('preserves instance alias when refreshing an existing managed group', () => {
    const cubeId = 'local/demo';
    const alias = 'Alpha';
    const inputMarker = makeMarker({
      id: 1,
      type: 'SugarCubes.CubeInput',
      cubeId,
      defaultAlias: 'Demo',
      instanceId: 'inst-target',
      alias,
    });
    const outputMarker = makeMarker({
      id: 2,
      type: 'SugarCubes.CubeOutput',
      cubeId,
      defaultAlias: 'Demo',
      instanceId: 'inst-target',
      alias,
    });
    const node = makeInternalNode(3);

    const links = {
      11: { id: 11, origin_id: inputMarker.id, origin_slot: 0, target_id: node.id, target_slot: 0 },
      12: {
        id: 12,
        origin_id: node.id,
        origin_slot: 0,
        target_id: outputMarker.id,
        target_slot: 0,
      },
    };
    inputMarker.outputs[0].links.push(11);
    node.inputs[0].link = 11;
    node.outputs[0].links.push(12);
    outputMarker.inputs[0].link = 12;

    const targetGroup = {
      title: alias,
      pos: [0, 0],
      size: [300, 240],
      properties: {
        sugarcubes: {
          schema: 5,
          managed: true,
          instance_id: 'inst-target',
          cube_id: cubeId,
          default_alias: 'Demo',
          instance_alias: alias,
          markers: { inputs: [inputMarker.id], outputs: [outputMarker.id] },
          nodes: [node.id],
          bounds: {
            x: 0,
            y: 0,
            w: 300,
            h: 240,
            padding: { x: 10, y: 10, top_extra: 16 },
            header: { height: 24 },
          },
        },
      },
    };

    const graph = {
      _nodes: [inputMarker, outputMarker, node],
      _groups: [targetGroup],
      links,
      remove: jest.fn(),
    };
    for (const entry of graph._nodes) {
      entry.graph = graph;
    }

    class LGraphGroup implements ComfyGroup {
      [key: string]: unknown;
      title: string;
      pos: number[];
      size: number[];
      properties = {};

      constructor(title = '') {
        this.title = title;
        this.pos = [0, 0];
        this.size = [640, 480];
      }
    }
    const manager = new InstanceManager({
      adapter: {
        getLiteGraph: () => ({ LGraphGroup }),
      },
      events: { emit: jest.fn() },
      scheduler: { raf: (callback) => (callback(0), 1) },
      requestDirtyRefresh: jest.fn(),
    });

    manager.refresh({ graph });

    const metadata = getGroupSugarcubes(targetGroup);
    expect(metadata!.instance_alias).toBe(alias);
    expect(targetGroup.title).toBe(alias);
  });

  test('recomputes stale managed bounds when content is outside group', () => {
    const cubeId = 'local/demo';
    const alias = 'Alpha';
    const inputMarker = makeMarker({
      id: 10,
      type: 'SugarCubes.CubeInput',
      cubeId,
      defaultAlias: 'Demo',
      instanceId: 'inst-target',
      alias,
    });
    const outputMarker = makeMarker({
      id: 11,
      type: 'SugarCubes.CubeOutput',
      cubeId,
      defaultAlias: 'Demo',
      instanceId: 'inst-target',
      alias,
    });
    const node = makeInternalNode(12);
    inputMarker.pos = [500, 2000];
    outputMarker.pos = [760, 2240];
    node.pos = [620, 2120];
    node.size = [260, 180];

    const links = {
      21: { id: 21, origin_id: inputMarker.id, origin_slot: 0, target_id: node.id, target_slot: 0 },
      22: {
        id: 22,
        origin_id: node.id,
        origin_slot: 0,
        target_id: outputMarker.id,
        target_slot: 0,
      },
    };
    inputMarker.outputs[0].links.push(21);
    node.inputs[0].link = 21;
    node.outputs[0].links.push(22);
    outputMarker.inputs[0].link = 22;

    const targetGroup = {
      title: alias,
      pos: [0, 0],
      size: [180, 120],
      properties: {
        sugarcubes: {
          schema: 5,
          managed: true,
          instance_id: 'inst-target',
          cube_id: cubeId,
          default_alias: 'Demo',
          instance_alias: alias,
          markers: { inputs: [inputMarker.id], outputs: [outputMarker.id] },
          nodes: [node.id],
          bounds: {
            x: 0,
            y: 0,
            w: 180,
            h: 120,
            padding: { x: 10, y: 10, top_extra: 16 },
            header: { height: 24 },
          },
        },
      },
    };

    const graph = {
      _nodes: [inputMarker, outputMarker, node],
      _groups: [targetGroup],
      links,
      remove: jest.fn(),
    };
    for (const entry of graph._nodes) {
      entry.graph = graph;
    }

    class LGraphGroup implements ComfyGroup {
      [key: string]: unknown;
      title: string;
      pos: number[];
      size: number[];
      properties = {};

      constructor(title = '') {
        this.title = title;
        this.pos = [0, 0];
        this.size = [640, 480];
      }
    }

    const manager = new InstanceManager({
      adapter: {
        getLiteGraph: () => ({ LGraphGroup }),
      },
      events: { emit: jest.fn() },
      scheduler: { raf: (callback) => (callback(0), 1) },
      requestDirtyRefresh: jest.fn(),
    });

    manager.refresh({ graph });

    const metadata = getGroupSugarcubes(targetGroup);
    const contentMinY = Math.min(inputMarker.pos[1], outputMarker.pos[1], node.pos[1]);
    const contentMaxY = Math.max(
      inputMarker.pos[1] + inputMarker.size[1],
      outputMarker.pos[1] + outputMarker.size[1],
      node.pos[1] + node.size[1],
    );
    const bounds = metadata!.bounds as CubeBoundsPayload;
    expect(bounds.y).toBeLessThanOrEqual(contentMinY);
    expect(bounds.y + bounds.h).toBeGreaterThanOrEqual(contentMaxY);
    expect(targetGroup.pos[1]).toBe(bounds.y);
    expect(targetGroup.size[1]).toBe(bounds.h);
  });
});
