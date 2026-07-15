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
import { computeDefinitionHash } from '../../web/comfyui/ui/graph/DirtyHasher.js';
import { BaselineResolver } from '../../web/comfyui/ui/graph/BaselineResolver.js';
import { BaselineStore } from '../../web/comfyui/ui/graph/BaselineStore.js';
import { DirtyTracker } from '../../web/comfyui/ui/graph/DirtyTracker.js';
import type { CubeGroupMetadataRecord } from '../../web/comfyui/ui/graph/GroupMetadata.js';
import type { ComfyNode } from '../../web/comfyui/ui/types/graph.js';
import type { UnknownRecord } from '../../web/comfyui/ui/types/common.js';

type MutableTestNode = ComfyNode & {
  pos: number[];
  size: number[];
  properties: UnknownRecord;
  mode?: number;
};

function makeGraph() {
  const nodes: MutableTestNode[] = [
    {
      id: 1,
      type: 'A',
      pos: [0, 0],
      size: [100, 50],
      widgets: [{ name: 'seed', value: 1 }],
      properties: { seed: 1 },
    },
    {
      id: 2,
      type: 'B',
      pos: [120, 0],
      size: [100, 50],
      widgets: [{ name: 'steps', value: 20 }],
      properties: {},
    },
  ];
  const links = [{ origin_id: 1, origin_slot: 0, target_id: 2, target_slot: 0, type: '*' }];
  const sugarcubes: CubeGroupMetadataRecord = {
    managed: true,
    instance_id: 'inst-1',
    cube_id: 'local/example-user/demo.cube',
    nodes: ['1', '2'],
    markers: { input: ['10'], output: ['11'] },
  };
  const group = {
    title: 'Cube Group',
    pos: [0, 0],
    size: [300, 200],
    properties: {
      sugarcubes,
    },
  };
  return { graph: { _nodes: nodes, links, _groups: [group] }, group, nodes };
}

function createTracker() {
  const baselineStore = new BaselineStore();
  const baselineResolver = new BaselineResolver({ baselineStore });
  const tracker = new DirtyTracker({ baselineStore, baselineResolver });
  return { tracker, baselineStore };
}

describe('cube dirty state', () => {
  test('refreshCubeDirtyState tracks dirty changes', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph();
    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
    expect(group.properties.sugarcubes.dirty).toBe(false);

    nodes[0].pos = [10, 10];
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.has('local/example-user/demo.cube')).toBe(false);
    expect(group.properties.sugarcubes.dirty).toBe(false);
    expect(group.properties.sugarcubes.cosmetic_dirty).toBe(true);
    const instance = tracker.instances.get('inst-1');
    expect(instance?.cosmeticDirty).toBe(true);
    expect(group.properties.sugarcubes.has_saveable_changes).toBe(false);
    expect(tracker.getSaveableCubeIds().size).toBe(0);
  });

  test('node execution mode changes mark implementation dirty', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph();
    tracker.markLocalBaseline({ graph, cubeIds: ['local/example-user/demo.cube'] });
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    nodes[0].mode = 4;
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.has('local/example-user/demo.cube')).toBe(true);
    expect(group.properties.sugarcubes.dirty).toBe(true);
    expect(group.properties.sugarcubes.implementation_dirty).toBe(true);
    expect(group.properties.sugarcubes.has_saveable_changes).toBe(true);
  });

  test('refreshCubeDirtyState ignores group-only moves', () => {
    const { tracker } = createTracker();
    const { graph, group } = makeGraph();
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    group.pos = [200, 150];
    group.size = [320, 210];
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.size).toBe(0);
    expect(group.properties.sugarcubes.dirty).toBe(false);
  });

  test('known cube title changes do not mark dirty after local baseline', () => {
    const { tracker, baselineStore } = createTracker();
    const { graph, group } = makeGraph();
    baselineStore.setDefinition('local/example-user/demo.cube', {
      hash: 'def-hash',
      status: 'ready',
    });
    tracker.markLocalBaseline({ graph, cubeIds: ['local/example-user/demo.cube'] });
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    group.title = 'Cube Group 2';
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.size).toBe(0);
    expect(group.properties.sugarcubes.dirty).toBe(false);
  });

  test('unknown cube title changes still mark dirty after local baseline', () => {
    const { tracker } = createTracker();
    const { graph, group } = makeGraph();
    tracker.markLocalBaseline({ graph, cubeIds: ['local/example-user/demo.cube'] });
    tracker.refresh({ graph, knownCubeIds: [] });

    group.title = 'Cube Group 2';
    const next = tracker.refresh({ graph, knownCubeIds: [] });

    expect(next.dirtyCubeIds.has('local/example-user/demo.cube')).toBe(true);
    expect(group.properties.sugarcubes.dirty).toBe(true);
  });

  test('group-only moves stay clean with definition hashes', () => {
    const { tracker, baselineStore } = createTracker();
    const { graph, group } = makeGraph();
    const definition = {
      nodes: [
        {
          symbol: 'node_a',
          class_type: 'A',
          inputs: { seed: 1 },
          layout: { pos: [0, 0], size: [100, 50] },
        },
        {
          symbol: 'node_b',
          class_type: 'B',
          inputs: { steps: 20 },
          layout: { pos: [120, 0], size: [100, 50] },
        },
      ],
      markers: [],
      connections: [
        {
          from: { symbol: 'node_a', slot: 0 },
          to: { symbol: 'node_b', input: '0' },
          type: '',
        },
      ],
    };

    graph._nodes[0].properties = { sugarcubes_symbol: 'node_a' };
    graph._nodes[1].properties = { sugarcubes_symbol: 'node_b' };
    const definitionHash = computeDefinitionHash(definition);
    baselineStore.setDefinition('local/example-user/demo.cube', {
      hash: definitionHash,
      status: 'ready',
    });
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    group.pos = [400, 220];
    group.size = [420, 260];
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.size).toBe(0);
    expect(group.properties.sugarcubes.dirty).toBe(false);
  });

  test('moving nodes relative to group still marks dirty', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph();
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    group.pos = [200, 150];
    group.size = [320, 210];
    nodes[0].pos = [30, 10];
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.has('local/example-user/demo.cube')).toBe(false);
    expect(group.properties.sugarcubes.dirty).toBe(false);
    expect(group.properties.sugarcubes.cosmetic_dirty).toBe(true);
    expect(group.properties.sugarcubes.has_saveable_changes).toBe(false);
    expect(tracker.getSaveableCubeIds().size).toBe(0);
  });

  test('surface value changes mark preset state without making implementation saveable', () => {
    const { tracker } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'A',
          pos: [0, 0],
          size: [100, 50],
          widgets: [{ name: 'cfg', value: 7 }],
          properties: { sugarcubes_symbol: '1' },
        },
      ],
      links: [],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              nodes: ['1'],
              markers: {},
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: '1.cfg',
                    symbol: '1',
                    input_name: 'cfg',
                    class_type: 'A',
                    value_type: 'number',
                  },
                ],
              },
              active_flavor_values: { '1.cfg': 7 },
            },
          },
        },
      ],
    };

    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });
    graph._nodes[0].widgets[0].value = 8;
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });
    const metadata = graph._groups[0].properties.sugarcubes as CubeGroupMetadataRecord;

    expect(next.dirtyCubeIds.size).toBe(0);
    expect(metadata.dirty).toBe(false);
    expect(metadata.implementation_dirty).toBe(false);
    expect(metadata.surface_values_changed).toBe(true);
    expect(metadata.has_saveable_changes).toBe(false);
    expect(tracker.getSaveableCubeIds().size).toBe(0);
  });

  test('seed surface value changes stay clean because seeds are volatile', () => {
    const { tracker, baselineStore } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'KSampler',
          pos: [0, 0],
          size: [100, 50],
          widgets: [{ name: 'seed', value: 111 }],
          properties: { sugarcubes_symbol: 'ksampler' },
        },
      ],
      links: [],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              cube_definition_key: 'local/example-user/demo.cube@1.0.0',
              nodes: ['1'],
              markers: {},
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'ksampler.seed',
                    symbol: 'ksampler',
                    input_name: 'seed',
                    class_type: 'KSampler',
                    value_type: 'number',
                  },
                ],
              },
              active_flavor_values: {},
            },
          },
        },
      ],
    };
    const definition = {
      cube: {
        surface: graph._groups[0].properties.sugarcubes.surface,
      },
      nodes: [
        {
          symbol: 'ksampler',
          class_type: 'KSampler',
          inputs: {},
          layout: { pos: [0, 0], size: [100, 50] },
        },
      ],
      markers: [],
      connections: [],
    };
    const definitionHash = computeDefinitionHash(definition);
    baselineStore.setDefinition('local/example-user/demo.cube@1.0.0', {
      hash: definitionHash,
      status: 'ready',
    });

    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });
    graph._nodes[0].widgets[0].value = 222;
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });
    const metadata = graph._groups[0].properties.sugarcubes as CubeGroupMetadataRecord;

    expect(next.dirtyCubeIds.size).toBe(0);
    expect(metadata.dirty).toBe(false);
    expect(metadata.implementation_dirty).toBe(false);
    expect(metadata.surface_values_changed).toBe(false);
    expect(metadata.has_saveable_changes).toBe(false);
  });

  test('refreshCubeDirtyState retains dirty_at until clean', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph();
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    nodes[0].properties.seed = 2;
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });
    const dirtyAt = group.properties.sugarcubes.dirty_at;

    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(group.properties.sugarcubes.dirty).toBe(true);
    expect(group.properties.sugarcubes.dirty_at).toBe(dirtyAt);
  });

  test('refreshCubeDirtyState reacts to property changes', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph();
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    nodes[0].properties.seed = 2;
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.has('local/example-user/demo.cube')).toBe(true);
    expect(group.properties.sugarcubes.dirty).toBe(true);
  });

  test('markCubesClean resets dirty flags', () => {
    const { tracker } = createTracker();
    const { graph, group } = makeGraph();
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });
    group.properties.sugarcubes.dirty = true;
    group.properties.sugarcubes.dirty_at = '2024-01-01T00:00:00Z';

    tracker.markClean({ graph, cubeIds: ['local/example-user/demo.cube'] });

    expect(group.properties.sugarcubes.dirty).toBe(false);
    expect(group.properties.sugarcubes.dirty_at).toBe(null);
    expect(tracker.getDirtyCubeIds().size).toBe(0);
  });

  test('definition hash keeps clean instances clean', () => {
    const { tracker, baselineStore } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'A',
          pos: [0, 0],
          size: [100, 50],
          widgets: [{ name: 'seed', value: 1 }],
          properties: { sugarcubes_symbol: 'node_a' },
        },
        {
          id: 10,
          type: 'SugarCubes.CubeInput',
          pos: [120, 0],
          size: [80, 40],
          widgets: [
            { name: 'cube_id', value: 'local/example-user/demo.cube' },
            { name: 'default_alias', value: 'Demo Cube' },
            { name: 'instance_id', value: 'inst-live' },
          ],
          properties: { sugarcubes_symbol: 'input_a' },
        },
      ],
      links: [],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              nodes: ['1'],
              markers: { inputs: ['10'] },
            },
          },
        },
      ],
    };

    const definition = {
      nodes: [
        {
          symbol: 'node_a',
          class_type: 'A',
          inputs: { seed: 1 },
          layout: { pos: [0, 0], size: [100, 50] },
        },
      ],
      markers: [
        {
          alias: 'input_a',
          class_type: 'SugarCubes.CubeInput',
          widget_values: {
            cube_id: 'local/example-user/demo.cube',
            default_alias: 'Demo Cube',
            instance_id: 'inst-definition',
          },
          layout: { pos: [120, 0], size: [80, 40] },
        },
      ],
      connections: [],
    };

    const definitionHash = computeDefinitionHash(definition);
    baselineStore.setDefinition('local/example-user/demo.cube', {
      hash: definitionHash,
      status: 'ready',
    });
    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
  });

  test('local baseline overrides definition hash', () => {
    const { tracker, baselineStore } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'A',
          pos: [0, 0],
          size: [100, 50],
          widgets: [{ name: 'seed', value: 2 }],
          properties: { sugarcubes_symbol: 'node_a' },
        },
        {
          id: 10,
          type: 'SugarCubes.CubeInput',
          pos: [120, 0],
          size: [80, 40],
          widgets: [{ name: 'cube_id', value: 'local/example-user/demo.cube' }],
          properties: { sugarcubes_symbol: 'input_a' },
        },
      ],
      links: [],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              nodes: ['1'],
              markers: { inputs: ['10'] },
            },
          },
        },
      ],
    };

    const definition = {
      nodes: [
        {
          symbol: 'node_a',
          class_type: 'A',
          inputs: { seed: 1 },
          layout: { pos: [0, 0], size: [100, 50] },
        },
      ],
      markers: [
        {
          alias: 'input_a',
          class_type: 'SugarCubes.CubeInput',
          widget_values: { cube_id: 'local/example-user/demo.cube' },
          layout: { pos: [120, 0], size: [80, 40] },
        },
      ],
      connections: [],
    };

    const definitionHash = computeDefinitionHash(definition);
    baselineStore.setDefinition('local/example-user/demo.cube', {
      hash: definitionHash,
      status: 'ready',
    });
    tracker.markLocalBaseline({ graph, cubeIds: ['local/example-user/demo.cube'] });
    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
  });

  test('local baseline still marks dirty after changes', () => {
    const { tracker } = createTracker();
    const { graph, group, nodes } = makeGraph();
    tracker.markLocalBaseline({ graph, cubeIds: ['local/example-user/demo.cube'] });
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    nodes[0].properties.seed = 2;
    const next = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(next.dirtyCubeIds.has('local/example-user/demo.cube')).toBe(true);
    expect(group.properties.sugarcubes.dirty).toBe(true);
  });

  test('missing symbols skip definition hash when definitions are ready', () => {
    const { tracker, baselineStore } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'A',
          pos: [0, 0],
          size: [100, 50],
          widgets: [{ name: 'seed', value: 1 }],
          properties: {},
        },
      ],
      links: [],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              nodes: ['1'],
              markers: { inputs: [] },
            },
          },
        },
      ],
    };

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
    expect((graph._groups[0]!.properties.sugarcubes as CubeGroupMetadataRecord).dirty).toBe(false);
  });

  test('missing symbols do not mark dirty when definition status is not ready', () => {
    const { tracker, baselineStore } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'A',
          pos: [0, 0],
          size: [100, 50],
          widgets: [{ name: 'seed', value: 1 }],
          properties: {},
        },
      ],
      links: [],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              nodes: ['1'],
              markers: { inputs: [] },
            },
          },
        },
      ],
    };

    baselineStore.setDefinition('local/example-user/demo.cube', { status: 'error' });
    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
    expect((graph._groups[0]!.properties.sugarcubes as CubeGroupMetadataRecord).dirty).toBe(false);
  });

  test('markClean stays clean when definitions are unavailable', () => {
    const { tracker, baselineStore } = createTracker();
    const { graph, group, nodes } = makeGraph();
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    nodes[0].properties.seed = 2;
    tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });
    expect(group.properties.sugarcubes.dirty).toBe(true);

    tracker.markClean({ graph, cubeIds: ['local/example-user/demo.cube'] });
    baselineStore.setDefinition('local/example-user/demo.cube', { status: 'missing' });
    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
    expect(group.properties.sugarcubes.dirty).toBe(false);
  });

  test('preloaded dirty flags are reset on first refresh', () => {
    const { tracker, baselineStore } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'A',
          pos: [0, 0],
          size: [100, 50],
          widgets: [{ name: 'seed', value: 1 }],
          properties: { sugarcubes_symbol: 'node_a' },
        },
        {
          id: 10,
          type: 'SugarCubes.CubeInput',
          pos: [120, 0],
          size: [80, 40],
          widgets: [{ name: 'cube_id', value: 'local/example-user/demo.cube' }],
          properties: { sugarcubes_symbol: 'input_a' },
        },
      ],
      links: [],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              nodes: ['1'],
              markers: { inputs: ['10'] },
              dirty: true,
              dirty_at: '2026-01-01T00:00:00Z',
            },
          },
        },
      ],
    };

    const definition = {
      nodes: [
        {
          symbol: 'node_a',
          class_type: 'A',
          inputs: { seed: 1 },
          layout: { pos: [0, 0], size: [100, 50] },
        },
      ],
      markers: [
        {
          alias: 'input_a',
          class_type: 'SugarCubes.CubeInput',
          widget_values: { cube_id: 'local/example-user/demo.cube' },
          layout: { pos: [120, 0], size: [80, 40] },
        },
      ],
      connections: [],
    };

    const definitionHash = computeDefinitionHash(definition);
    baselineStore.setDefinition('local/example-user/demo.cube', {
      hash: definitionHash,
      status: 'ready',
    });
    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
    expect((graph._groups[0]!.properties.sugarcubes as CubeGroupMetadataRecord).dirty).toBe(false);
  });

  test('definition hash ignores volatile widget and layout noise', () => {
    const { tracker, baselineStore } = createTracker();
    const graph = {
      _nodes: [
        {
          id: 1,
          type: 'CheckpointLoader',
          pos: [0, 0],
          size: [260, 140],
          widgets: [
            { name: 'ckpt_name', value: 'model.safetensors' },
            { name: 'control_after_generate', value: 'randomize' },
          ],
          properties: { sugarcubes_symbol: 'checkpoint' },
          inputs: [{ name: 'model_in' }],
        },
        {
          id: 10,
          type: 'SugarCubes.CubeInput',
          pos: [120, 0],
          size: [80, 40],
          widgets: [
            { name: 'cube_id', value: 'local/example-user/demo.cube' },
            { name: 'default_alias', value: 'Demo Cube' },
          ],
          properties: { sugarcubes_symbol: 'input_a' },
        },
      ],
      links: [{ origin_id: 1, origin_slot: 0, target_id: 10, target_slot: 0, type: 'MODEL' }],
      _groups: [
        {
          title: 'Cube Group',
          pos: [0, 0],
          size: [300, 200],
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'local/example-user/demo.cube',
              nodes: ['1'],
              markers: { inputs: ['10'] },
            },
          },
        },
      ],
    };

    const definition = {
      nodes: [
        {
          symbol: 'checkpoint',
          class_type: 'CheckpointLoader',
          inputs: { ckpt_name: 'model.safetensors' },
          layout: { pos: [0, 0], size: [220, 120] },
        },
      ],
      markers: [
        {
          alias: 'input_a',
          class_type: 'SugarCubes.CubeInput',
          widget_values: {
            cube_id: 'local/example-user/demo.cube',
            default_alias: 'Demo Cube',
          },
          layout: { pos: [120, 0], size: [80, 40] },
        },
      ],
      connections: [
        {
          from: { symbol: 'checkpoint', slot: 0 },
          to: { symbol: 'input_a', input: '0' },
          type: '',
        },
      ],
    };

    const definitionHash = computeDefinitionHash(definition);
    baselineStore.setDefinition('local/example-user/demo.cube', {
      hash: definitionHash,
      status: 'ready',
    });
    const result = tracker.refresh({ graph, knownCubeIds: ['local/example-user/demo.cube'] });

    expect(result.dirtyCubeIds.size).toBe(0);
    expect((graph._groups[0]!.properties.sugarcubes as CubeGroupMetadataRecord).dirty).toBe(false);
  });
});
