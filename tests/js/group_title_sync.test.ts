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
import { InstanceBuilder } from '../../frontend/comfyui/ui/graph/InstanceBuilder.js';
import { InstanceManager } from '../../frontend/comfyui/ui/graph/InstanceManager.js';
import { syncInstanceAlias } from '../../frontend/comfyui/ui/graph/InstanceAliasSync.js';
import { readWidgetValue } from '../../frontend/comfyui/ui/graph/Markers.js';
import type { CubeGroupMetadataRecord } from '../../frontend/comfyui/ui/graph/GroupMetadata.js';
import type { InstanceAdapter } from '../../frontend/comfyui/ui/graph/InstanceManager.js';
import type {
  ComfyGraph,
  ComfyGroup,
  ComfyInput,
  ComfyLink,
  ComfyNode,
  ComfyOutput,
  ComfyWidget,
} from '../../frontend/comfyui/ui/types/graph.js';

interface TestGroup extends ComfyGroup {
  title: string;
  pos: number[];
  size: number[];
  properties: { sugarcubes?: CubeGroupMetadataRecord };
  __sugarcubes_imported?: boolean;
  getBounding?(): number[];
}

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

interface TestGraph extends ComfyGraph {
  _nodes: TestNode[];
  _groups: TestGroup[];
  links: Record<string, ComfyLink>;
  add(item: ComfyNode | ComfyGroup): void;
  remove(item: ComfyNode | ComfyGroup): void;
}

function makeAdapter(): InstanceAdapter {
  class LGraphGroup implements TestGroup {
    [key: string]: unknown;
    title: string;
    pos: number[];
    size: number[];
    properties: { sugarcubes?: CubeGroupMetadataRecord } = {};

    constructor(title = '') {
      this.title = title;
      this.pos = [0, 0];
      this.size = [0, 0];
    }
  }
  return {
    getLiteGraph: () => ({ LGraphGroup }),
    getConsole: () => ({ warn() {} }),
  };
}

function groupMetadata(group: TestGroup): CubeGroupMetadataRecord {
  const metadata = group.properties.sugarcubes;
  if (!metadata) {
    throw new Error(`Missing SugarCubes metadata for ${group.title}`);
  }
  return metadata;
}

function widget(node: TestNode, name: string): ComfyWidget {
  const match = node.widgets.find((entry) => entry.name === name);
  if (!match) {
    throw new Error(`Missing ${name} widget on node ${node.id}`);
  }
  return match;
}

function makeGraph({
  groupTitle = 'New Name',
  markerName = 'Old Name',
  existingInstanceId = '',
}: { groupTitle?: string; markerName?: string; existingInstanceId?: string } = {}) {
  const graph: TestGraph = {
    _nodes: [],
    _groups: [],
    links: {
      1: {
        id: 1,
        origin_id: 1,
        origin_slot: 0,
        target_id: 2,
        target_slot: 0,
        type: '*',
      },
    },
    add(item) {
      this._groups.push(item as TestGroup);
    },
    remove() {},
  };

  const markers: TestNode[] = [
    {
      id: 1,
      type: 'SugarCubes.CubeInput',
      pos: [10, 10],
      size: [80, 40],
      inputs: [],
      outputs: [{ type: '*', links: [1] }],
      widgets: [
        { name: 'cube_id', value: 'local/example-user/demo.cube' },
        { name: 'default_alias', value: markerName },
        { name: 'instance_alias', value: markerName },
        { name: 'instance_id', value: '' },
      ],
      graph,
    },
    {
      id: 2,
      type: 'SugarCubes.CubeOutput',
      pos: [120, 10],
      size: [80, 40],
      inputs: [{ type: '*', link: 1 }],
      outputs: [],
      widgets: [
        { name: 'cube_id', value: 'local/example-user/demo.cube' },
        { name: 'default_alias', value: markerName },
        { name: 'instance_alias', value: markerName },
        { name: 'instance_id', value: '' },
      ],
      graph,
    },
  ];

  const group: TestGroup = {
    title: groupTitle,
    pos: [0, 0],
    size: [240, 120],
    properties: {},
  };
  if (existingInstanceId) {
    group.properties.sugarcubes = {
      managed: true,
      instance_id: existingInstanceId,
      cube_id: 'local/example-user/demo.cube',
      default_alias: groupTitle,
      instance_alias: groupTitle,
      markers: { inputs: [1], outputs: [2] },
      nodes: [],
    };
  }

  graph._nodes = markers;
  graph._groups = [group];
  return { graph, group, markers };
}

function makeDuplicateSpawnGraph({
  aliases = ['Demo', 'Demo'],
  importedIndexes = [1],
}: { aliases?: string[]; importedIndexes?: number[] } = {}) {
  const graph: TestGraph = {
    _nodes: [],
    _groups: [],
    links: {},
    add(item: ComfyNode | ComfyGroup) {
      this._groups.push(item as TestGroup);
    },
    remove: jest.fn(),
  };
  const cubeId = 'local/example-user/demo.cube';
  const defaultAlias = 'Demo';
  let nextLinkId = 1;

  const makeMarker = ({
    id,
    type,
    pos,
    instanceId,
    alias,
    inputLink = null,
    outputLinks = [],
  }: {
    id: number;
    type: string;
    pos: number[];
    instanceId: string;
    alias: string;
    inputLink?: number | null;
    outputLinks?: number[];
  }): TestNode => ({
    id,
    type,
    pos,
    size: [80, 40],
    inputs: type === 'SugarCubes.CubeOutput' ? [{ type: '*', link: inputLink }] : [],
    outputs: type === 'SugarCubes.CubeInput' ? [{ type: '*', links: outputLinks }] : [],
    widgets: [
      { name: 'cube_id', value: cubeId },
      { name: 'default_alias', value: defaultAlias },
      { name: 'instance_alias', value: alias },
      { name: 'instance_id', value: instanceId },
    ],
    graph,
  });

  const instanceIds = aliases.map((_, index) => `inst-${String.fromCharCode(97 + index)}`);
  aliases.forEach((alias, index) => {
    const baseId = index * 10;
    const linkIn = nextLinkId++;
    const linkOut = nextLinkId++;
    const input = makeMarker({
      id: baseId + 1,
      type: 'SugarCubes.CubeInput',
      pos: [0, index * 120],
      instanceId: instanceIds[index],
      alias,
      outputLinks: [linkIn],
    });
    const node: TestNode = {
      id: baseId + 2,
      type: 'KSampler',
      pos: [120, index * 120],
      size: [120, 60],
      inputs: [{ type: '*', link: linkIn }],
      outputs: [{ type: '*', links: [linkOut] }],
      widgets: [],
      graph,
    };
    const output = makeMarker({
      id: baseId + 3,
      type: 'SugarCubes.CubeOutput',
      pos: [280, index * 120],
      instanceId: instanceIds[index],
      alias,
      inputLink: linkOut,
    });
    graph.links[linkIn] = {
      id: linkIn,
      origin_id: input.id,
      origin_slot: 0,
      target_id: node.id,
      target_slot: 0,
      type: '*',
    };
    graph.links[linkOut] = {
      id: linkOut,
      origin_id: node.id,
      origin_slot: 0,
      target_id: output.id,
      target_slot: 0,
      type: '*',
    };
    graph._nodes.push(input, node, output);

    const group: TestGroup = {
      title: alias,
      pos: [-20, index * 120 - 60],
      size: [420, 160],
      properties: {
        sugarcubes: {
          managed: true,
          instance_id: instanceIds[index],
          cube_id: cubeId,
          default_alias: defaultAlias,
          instance_alias: alias,
          markers: { inputs: [input.id], outputs: [output.id] },
          nodes: [node.id],
        },
      },
    };
    if (importedIndexes.includes(index)) {
      group.__sugarcubes_imported = true;
    }
    graph._groups.push(group);
  });

  return { graph, instanceIds };
}

describe('InstanceManager group title sync', () => {
  test('uses marker alias and updates group title', () => {
    const { graph, group, markers } = makeGraph();
    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-1' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    const sugarcubes = groupMetadata(group);
    expect(sugarcubes.default_alias).toBe('Old Name');
    expect(sugarcubes.instance_alias).toBe('Old Name');
    expect(sugarcubes.instance_id).toBe('inst-1');
    expect(group.title).toBe('Old Name');
    expect(readWidgetValue(markers[0], 'default_alias')).toBe('Old Name');
    expect(readWidgetValue(markers[1], 'default_alias')).toBe('Old Name');
    expect(readWidgetValue(markers[0], 'instance_alias')).toBe('Old Name');
    expect(readWidgetValue(markers[1], 'instance_alias')).toBe('Old Name');
    expect(readWidgetValue(markers[0], 'instance_id')).toBe('inst-1');
    expect(readWidgetValue(markers[1], 'instance_id')).toBe('inst-1');
  });

  test('syncs marker alias rename across group and markers', () => {
    const { graph, group, markers } = makeGraph({ groupTitle: 'Old Name', markerName: 'Old Name' });
    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-2' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    widget(markers[0], 'instance_alias').value = 'Fresh Name';

    syncInstanceAlias({
      graph,
      group,
      metadata: groupMetadata(group),
      cubeId: 'local/example-user/demo.cube',
      instanceAlias: 'Fresh Name',
    });
    manager.refresh({ graph });

    expect(group.title).toBe('Fresh Name');
    expect(readWidgetValue(markers[0], 'default_alias')).toBe('Old Name');
    expect(readWidgetValue(markers[1], 'default_alias')).toBe('Old Name');
    expect(readWidgetValue(markers[0], 'instance_alias')).toBe('Fresh Name');
    expect(readWidgetValue(markers[1], 'instance_alias')).toBe('Fresh Name');
  });

  test('keeps existing instance_id when markers disagree', () => {
    const { graph, group, markers } = makeGraph({
      groupTitle: 'Legacy Name',
      markerName: 'Legacy Name',
      existingInstanceId: 'inst-existing',
    });
    widget(markers[0], 'instance_id').value = 'inst-a';
    widget(markers[1], 'instance_id').value = 'inst-b';
    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-new' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    const sugarcubes = groupMetadata(group);
    expect(sugarcubes.instance_id).toBe('inst-existing');
    expect(readWidgetValue(markers[0], 'instance_id')).toBe('inst-existing');
    expect(readWidgetValue(markers[1], 'instance_id')).toBe('inst-existing');
  });

  test('canonicalizes instance_id when markers disagree and metadata is missing', () => {
    const { graph, group, markers } = makeGraph({
      groupTitle: 'Mismatch Name',
      markerName: 'Mismatch Name',
    });
    widget(markers[0], 'instance_id').value = 'b';
    widget(markers[1], 'instance_id').value = 'a';
    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-new' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    const sugarcubes = groupMetadata(group);
    expect(sugarcubes.instance_id).toBe('a');
    expect(readWidgetValue(markers[0], 'instance_id')).toBe('a');
    expect(readWidgetValue(markers[1], 'instance_id')).toBe('a');
    expect(sugarcubes.instance_alias).toBe('Mismatch Name');
  });

  test('normalizes managed group bounds from content during refresh', () => {
    const { graph, group } = makeGraph({
      groupTitle: 'Stable Bounds',
      markerName: 'Stable Bounds',
      existingInstanceId: 'inst-stable',
    });
    group.pos = [-120, -90];
    group.size = [520, 360];
    groupMetadata(group).bounds = {
      x: -120,
      y: -90,
      w: 520,
      h: 360,
      padding: { x: 9, y: 11, top_extra: 4 },
      header: { height: 28 },
    };

    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-stable' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    expect(group.pos).toEqual([0, -59]);
    expect(group.size).toEqual([210, 119]);
    expect(groupMetadata(group).bounds).toEqual({
      x: 0,
      y: -59,
      w: 210,
      h: 119,
      padding: { x: 9, y: 11, top_extra: 4 },
      header: { height: 28 },
    });
  });

  test('normalizes reusable unmanaged group bounds during initial attach', () => {
    const { graph, group } = makeGraph({
      groupTitle: 'Reusable Bounds',
      markerName: 'Reusable Bounds',
    });
    group.pos = [0, -60];
    group.size = [620, 500];
    group.properties = {};

    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-reuse' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    expect(group.pos).toEqual([0, -50]);
    expect(group.size).toEqual([210, 110]);
    expect(groupMetadata(group).bounds).toEqual({
      x: 0,
      y: -50,
      w: 210,
      h: 110,
      padding: { x: 2, y: 2, top_extra: 0 },
      header: { height: 32 },
    });
  });

  test('expands tight unmanaged reusable bounds to minimum import margins', () => {
    const { graph, group } = makeGraph({
      groupTitle: 'Tight Reusable',
      markerName: 'Tight Reusable',
    });
    group.pos = [8, 8];
    group.size = [196, 48];
    group.properties = {};

    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-tight' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    expect(group.pos).toEqual([0, -50]);
    expect(group.size).toEqual([210, 110]);
    expect(groupMetadata(group).bounds).toEqual({
      x: 0,
      y: -50,
      w: 210,
      h: 110,
      padding: { x: 2, y: 2, top_extra: 0 },
      header: { height: 32 },
    });
  });

  test('expands tight unmanaged bounds using visual node bounds for header clearance', () => {
    const { graph, group, markers } = makeGraph({
      groupTitle: 'Visual Bounds',
      markerName: 'Visual Bounds',
    });
    group.pos = [8, 8];
    group.size = [196, 48];
    group.properties = {};
    for (const marker of markers) {
      marker.getBounding = () => [
        marker.pos[0],
        marker.pos[1] - 30,
        marker.size[0],
        marker.size[1] + 30,
      ];
    }

    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-visual' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    expect(group.pos).toEqual([0, -80]);
    expect(group.size).toEqual([210, 140]);
    expect(groupMetadata(group).bounds).toEqual({
      x: 0,
      y: -80,
      w: 210,
      h: 140,
      padding: { x: 2, y: 2, top_extra: 0 },
      header: { height: 32 },
    });
  });

  test('removes duplicate managed groups for the same instance', () => {
    const { graph, group, markers } = makeGraph({
      groupTitle: 'Dup Name',
      markerName: 'Dup Name',
      existingInstanceId: 'inst-dup',
    });
    const duplicateGroup = {
      title: 'Dup Name',
      pos: [0, 0],
      size: [240, 120],
      properties: {
        sugarcubes: {
          managed: true,
          instance_id: 'inst-dup',
          cube_id: 'local/example-user/demo.cube',
          default_alias: 'Dup Name',
          instance_alias: 'Dup Name',
          markers: { inputs: [markers[0].id], outputs: [markers[1].id] },
          nodes: [],
        },
      },
    };
    graph._groups = [group, duplicateGroup];
    graph.remove = jest.fn();
    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-dup' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    expect(graph.remove).toHaveBeenCalledTimes(1);
    expect(graph.remove).toHaveBeenCalledWith(group);
    expect(graph.remove).not.toHaveBeenCalledWith(duplicateGroup);
  });

  test('keeps distinct aliases for instances sharing the same cube_id', () => {
    const graph: TestGraph = {
      _nodes: [],
      _groups: [],
      links: {},
      add(group: ComfyNode | ComfyGroup) {
        this._groups.push(group as TestGroup);
      },
      remove() {},
    };
    const cubeId = 'local/example-user/demo.cube';
    const defaultAlias = 'Demo';

    const markers: TestNode[] = [
      {
        id: 1,
        type: 'SugarCubes.CubeInput',
        pos: [0, 0],
        size: [80, 40],
        inputs: [],
        outputs: [{ type: '*', links: [1] }],
        widgets: [
          { name: 'cube_id', value: cubeId },
          { name: 'default_alias', value: defaultAlias },
          { name: 'instance_alias', value: 'Alpha' },
          { name: 'instance_id', value: 'inst-a' },
        ],
        graph,
      },
      {
        id: 2,
        type: 'SugarCubes.CubeOutput',
        pos: [120, 0],
        size: [80, 40],
        outputs: [],
        inputs: [{ type: '*', link: 1 }],
        widgets: [
          { name: 'cube_id', value: cubeId },
          { name: 'default_alias', value: defaultAlias },
          { name: 'instance_alias', value: 'Alpha' },
          { name: 'instance_id', value: 'inst-a' },
        ],
        graph,
      },
      {
        id: 3,
        type: 'SugarCubes.CubeInput',
        pos: [0, 120],
        size: [80, 40],
        inputs: [],
        outputs: [{ type: '*', links: [2] }],
        widgets: [
          { name: 'cube_id', value: cubeId },
          { name: 'default_alias', value: defaultAlias },
          { name: 'instance_alias', value: 'Beta' },
          { name: 'instance_id', value: 'inst-b' },
        ],
        graph,
      },
      {
        id: 4,
        type: 'SugarCubes.CubeOutput',
        pos: [120, 120],
        size: [80, 40],
        outputs: [],
        inputs: [{ type: '*', link: 2 }],
        widgets: [
          { name: 'cube_id', value: cubeId },
          { name: 'default_alias', value: defaultAlias },
          { name: 'instance_alias', value: 'Beta' },
          { name: 'instance_id', value: 'inst-b' },
        ],
        graph,
      },
    ];

    graph.links = {
      1: { id: 1, origin_id: 1, origin_slot: 0, target_id: 2, target_slot: 0, type: '*' },
      2: { id: 2, origin_id: 3, origin_slot: 0, target_id: 4, target_slot: 0, type: '*' },
    };
    graph._nodes = markers;

    const instanceBuilder = new InstanceBuilder({ instanceIdFactory: () => 'inst-fallback' });
    const manager = new InstanceManager({ adapter: makeAdapter(), instanceBuilder });

    manager.refresh({ graph });

    expect(graph._groups).toHaveLength(2);
    const groupsByAlias = new Map(
      graph._groups.map((group) => [group.properties?.sugarcubes?.instance_alias, group]),
    );
    const groupAlpha = groupsByAlias.get('Alpha');
    const groupBeta = groupsByAlias.get('Beta');

    expect(groupAlpha?.properties?.sugarcubes?.default_alias).toBe(defaultAlias);
    expect(groupBeta?.properties?.sugarcubes?.default_alias).toBe(defaultAlias);
    expect(groupAlpha?.title).toBe('Alpha');
    expect(groupBeta?.title).toBe('Beta');

    expect(readWidgetValue(markers[0], 'instance_alias')).toBe('Alpha');
    expect(readWidgetValue(markers[1], 'instance_alias')).toBe('Alpha');
    expect(readWidgetValue(markers[2], 'instance_alias')).toBe('Beta');
    expect(readWidgetValue(markers[3], 'instance_alias')).toBe('Beta');
  });

  test('keeps the existing default alias and suffixes a newly imported duplicate', () => {
    const { graph, instanceIds } = makeDuplicateSpawnGraph();
    const manager = new InstanceManager({ adapter: makeAdapter() });

    manager.refresh({ graph });

    const aliasesByInstance = new Map(
      graph._groups.map((group) => [
        group.properties?.sugarcubes?.instance_id,
        group.properties?.sugarcubes?.instance_alias,
      ]),
    );
    expect(aliasesByInstance.get(instanceIds[0])).toBe('Demo');
    expect(aliasesByInstance.get(instanceIds[1])).toBe('Demo 2');

    const markersByInstance = new Map();
    for (const node of graph._nodes.filter((entry) => entry.widgets)) {
      const instanceId = readWidgetValue(node, 'instance_id');
      const aliases = markersByInstance.get(instanceId) ?? new Set();
      aliases.add(readWidgetValue(node, 'instance_alias'));
      markersByInstance.set(instanceId, aliases);
    }
    expect([...markersByInstance.get(instanceIds[0])]).toEqual(['Demo']);
    expect([...markersByInstance.get(instanceIds[1])]).toEqual(['Demo 2']);
  });

  test('force refresh normalizes stale bounds even when the instance signature is unchanged', () => {
    const { graph } = makeDuplicateSpawnGraph({ aliases: ['Demo'], importedIndexes: [0] });
    const manager = new InstanceManager({ adapter: makeAdapter() });

    manager.refresh({ graph });
    const group = graph._groups[0];
    expect(group.pos).toEqual([-10, -60]);
    expect(group.size).toEqual([380, 130]);

    group.pos = [-20, -60];
    group.size = [420, 160];
    groupMetadata(group).bounds = {
      ...(groupMetadata(group).bounds as Record<string, unknown>),
      x: -20,
      y: -60,
      w: 420,
      h: 160,
    };

    manager.refresh({ graph });
    expect(group.pos).toEqual([-20, -60]);
    expect(group.size).toEqual([420, 160]);

    manager.refresh({ graph, force: true });
    expect(group.pos).toEqual([-10, -60]);
    expect(group.size).toEqual([380, 130]);
    expect(groupMetadata(group).bounds).toMatchObject({ x: -10, y: -60, w: 380, h: 130 });
  });

  test('scheduled force refresh carries the force flag into the next frame', () => {
    const { graph } = makeDuplicateSpawnGraph({ aliases: ['Demo'], importedIndexes: [0] });
    const manager = new InstanceManager({
      adapter: makeAdapter(),
      scheduler: { raf: (callback) => (callback(0), 1) },
    });

    manager.refresh({ graph });
    const group = graph._groups[0];
    group.pos = [-20, -60];
    group.size = [420, 160];
    groupMetadata(group).bounds = {
      ...(groupMetadata(group).bounds as Record<string, unknown>),
      x: -20,
      y: -60,
      w: 420,
      h: 160,
    };

    manager.scheduleRefresh({ graph, reason: 'test', force: true });

    expect(group.pos).toEqual([-10, -60]);
    expect(group.size).toEqual([380, 130]);
  });

  test('lets a newly imported duplicate use the default alias when the existing copy was renamed', () => {
    const { graph, instanceIds } = makeDuplicateSpawnGraph({ aliases: ['Custom', 'Demo'] });
    const manager = new InstanceManager({ adapter: makeAdapter() });

    manager.refresh({ graph });

    const aliasesByInstance = new Map(
      graph._groups.map((group) => [
        group.properties?.sugarcubes?.instance_id,
        group.properties?.sugarcubes?.instance_alias,
      ]),
    );
    expect(aliasesByInstance.get(instanceIds[0])).toBe('Custom');
    expect(aliasesByInstance.get(instanceIds[1])).toBe('Demo');
  });

  test('allocates deterministic suffixes when two duplicates are imported after an existing copy', () => {
    const { graph, instanceIds } = makeDuplicateSpawnGraph({
      aliases: ['Demo', 'Demo', 'Demo'],
      importedIndexes: [1, 2],
    });
    const manager = new InstanceManager({ adapter: makeAdapter() });

    manager.refresh({ graph });

    const aliasesByInstance = new Map(
      graph._groups.map((group) => [
        group.properties?.sugarcubes?.instance_id,
        group.properties?.sugarcubes?.instance_alias,
      ]),
    );
    expect(aliasesByInstance.get(instanceIds[0])).toBe('Demo');
    expect(aliasesByInstance.get(instanceIds[1])).toBe('Demo 2');
    expect(aliasesByInstance.get(instanceIds[2])).toBe('Demo 3');
  });
});
