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
import {
  CUBE_INSTANCE_HEADER_HEIGHT,
  CUBE_INSTANCE_PADDING,
  CUBE_INSTANCE_TOP_EXTRA,
} from '../../frontend/comfyui/ui/graph/CubeBounds.js';
import { InstanceBuilder } from '../../frontend/comfyui/ui/graph/InstanceBuilder.js';
import { setGroupSugarcubes } from '../../frontend/comfyui/ui/graph/GroupMetadata.js';
import { CubeInstanceIndex } from '../../frontend/comfyui/ui/layout/CubeInstanceIndex.js';
import type { CubeGroupMetadataRecord } from '../../frontend/comfyui/ui/graph/GroupMetadata.js';
import type {
  ComfyGraph,
  ComfyGroup,
  ComfyLink,
  ComfyNode,
  ComfyWidget,
  GraphId,
  NumericVector,
} from '../../frontend/comfyui/ui/types/graph.js';

interface MarkerOptions {
  id: GraphId;
  type: string;
  defaultAlias: string;
  cubeId?: string;
  instanceId?: string;
  alias?: string;
  pos: NumericVector;
  size: NumericVector;
}

function makeWidget(name: string, value: unknown): ComfyWidget {
  return { name, value };
}

function makeMarker({
  id,
  type,
  defaultAlias,
  cubeId,
  instanceId,
  alias,
  pos,
  size,
}: MarkerOptions): ComfyNode {
  const widgets = [makeWidget('default_alias', defaultAlias)];
  if (cubeId) {
    widgets.push(makeWidget('cube_id', cubeId));
  }
  if (instanceId) {
    widgets.push(makeWidget('instance_id', instanceId));
  }
  if (alias) {
    widgets.push(makeWidget('instance_alias', alias));
  }
  return {
    id,
    type,
    pos,
    size,
    widgets,
  };
}

function makeGroup(
  metadata: CubeGroupMetadataRecord,
  {
    bounding,
    pos,
    size,
  }: { bounding?: NumericVector; pos?: NumericVector; size?: NumericVector } = {},
): ComfyGroup {
  const group = {
    _bounding: bounding,
    pos,
    size,
    properties: {},
  };
  setGroupSugarcubes(group, metadata);
  return group;
}

function buildGraph({
  nodes,
  groups,
  links = [],
}: {
  nodes: ComfyNode[];
  groups: ComfyGroup[];
  links?: ComfyLink[];
}): ComfyGraph {
  return {
    _nodes: nodes,
    _groups: groups,
    links,
  };
}

describe('cube instance index', () => {
  test('indexes by instance id, cube id, and marker id', () => {
    const marker = makeMarker({
      id: 10,
      type: 'SugarCubes.CubeInput',
      defaultAlias: 'Alpha',
      cubeId: 'cube-1',
      instanceId: 'inst-1',
      pos: [0, 0],
      size: [10, 10],
    });
    const group = makeGroup(
      {
        managed: true,
        instance_id: 'inst-1',
        cube_id: 'cube-1',
        markers: { inputs: [10], outputs: [] },
        nodes: [],
      },
      { bounding: [1, 2, 3, 4] },
    );
    const graph = buildGraph({ nodes: [marker], groups: [group] });
    const index = new CubeInstanceIndex({ graph });

    const entry = index.instanceById.get('inst-1');
    expect(entry).toBeTruthy();
    expect(entry!.group).toBe(group);
    expect(index.instanceByCubeId.get('cube-1')).toBe(entry);
    expect(index.instanceByMarkerId.get('10')).toBe(entry);
  });

  test('prefers metadata bounds without mutating canonical metadata', () => {
    const marker = makeMarker({
      id: 20,
      type: 'SugarCubes.CubeOutput',
      defaultAlias: 'Beta',
      cubeId: 'cube-2',
      instanceId: 'inst-2',
      pos: [0, 0],
      size: [10, 10],
    });
    const metadata = {
      managed: true,
      instance_id: 'inst-2',
      cube_id: 'cube-2',
      bounds: { x: 9, y: 8, w: 7, h: 6, padding: { x: 1 } },
      markers: { inputs: [], outputs: [20] },
      nodes: [],
    };
    const group = makeGroup(metadata, { bounding: [1, 2, 3, 4] });
    const graph = buildGraph({ nodes: [marker], groups: [group] });
    const index = new CubeInstanceIndex({ graph });
    const entry = index.instanceById.get('inst-2');
    expect(entry!.bounds).toEqual({
      x: 9,
      y: 8,
      w: 7,
      h: 6,
      padding: {
        x: 1,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    });
    expect(metadata.bounds).toEqual({ x: 9, y: 8, w: 7, h: 6, padding: { x: 1 } });
  });

  test('does not mutate moved group position while indexing', () => {
    const marker = makeMarker({
      id: 21,
      type: 'SugarCubes.CubeOutput',
      defaultAlias: 'Beta Moved',
      cubeId: 'cube-2b',
      instanceId: 'inst-2b',
      pos: [40, 60],
      size: [10, 10],
    });
    const metadata = {
      managed: true,
      instance_id: 'inst-2b',
      cube_id: 'cube-2b',
      bounds: { x: 9, y: 8, w: 70, h: 60, padding: { x: 1 } },
      markers: { inputs: [], outputs: [21] },
      nodes: [],
    };
    const group = makeGroup(metadata, {
      bounding: [80, 90, 70, 60],
      pos: new Float32Array([80, 90]),
      size: new Float32Array([70, 60]),
    });
    const graph = buildGraph({ nodes: [marker], groups: [group] });

    new CubeInstanceIndex({ graph });

    expect(Array.from(group.pos!)).toEqual([80, 90]);
    expect(Array.from(group.size!)).toEqual([70, 60]);
  });

  test('uses group bounds when metadata bounds missing', () => {
    const marker = makeMarker({
      id: 30,
      type: 'SugarCubes.CubeInput',
      defaultAlias: 'Gamma',
      cubeId: 'cube-3',
      instanceId: 'inst-3',
      pos: [0, 0],
      size: [10, 10],
    });
    const group = makeGroup(
      {
        managed: true,
        instance_id: 'inst-3',
        cube_id: 'cube-3',
        markers: { inputs: [30], outputs: [] },
        nodes: [],
      },
      { bounding: [5, 6, 7, 8] },
    );
    const graph = buildGraph({ nodes: [marker], groups: [group] });
    const index = new CubeInstanceIndex({ graph });
    const entry = index.instanceById.get('inst-3');
    expect(entry!.bounds).toEqual({
      x: 5,
      y: 6,
      w: 7,
      h: 8,
      padding: {
        x: CUBE_INSTANCE_PADDING.x,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    });
  });

  test('computes bounds from nodes and markers when group is missing', () => {
    const marker = makeMarker({
      id: 40,
      type: 'SugarCubes.CubeInput',
      defaultAlias: 'Delta',
      cubeId: 'cube-4',
      instanceId: 'inst-4',
      pos: [10, 10],
      size: [10, 10],
    });
    const graph = buildGraph({ nodes: [marker], groups: [] });
    const index = new CubeInstanceIndex({ graph });
    const entry = index.instanceById.get('inst-4');
    const padX = CUBE_INSTANCE_PADDING.x;
    const padY = CUBE_INSTANCE_PADDING.y;
    const padTop = padY + CUBE_INSTANCE_TOP_EXTRA;
    const padBottom = padY;
    expect(entry!.bounds).toEqual({
      x: 10 - padX,
      y: 10 - padTop - CUBE_INSTANCE_HEADER_HEIGHT,
      w: 10 + padX * 2,
      h: 10 + padTop + padBottom + CUBE_INSTANCE_HEADER_HEIGHT,
    });
  });

  test('matches groups by marker signature when instance ids differ', () => {
    const marker = makeMarker({
      id: 50,
      type: 'SugarCubes.CubeInput',
      defaultAlias: 'Echo',
      cubeId: 'cube-5',
      pos: [0, 0],
      size: [10, 10],
    });
    const group = makeGroup(
      {
        managed: true,
        instance_id: 'inst-group',
        cube_id: 'cube-5',
        markers: { inputs: [50], outputs: [] },
        nodes: [],
      },
      { bounding: [2, 2, 4, 4] },
    );
    const graph = buildGraph({ nodes: [marker], groups: [group] });
    const instanceBuilder = new InstanceBuilder({
      instanceIdFactory: () => 'inst-generated',
    });
    const index = new CubeInstanceIndex({ graph, instanceBuilder });
    const entry = index.instanceById.get('inst-generated');
    expect(entry!.group).toBe(group);
  });
});
