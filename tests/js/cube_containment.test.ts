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
  CubeContainmentService,
  type ContainmentInstance,
} from '../../web/comfyui/ui/layout/CubeContainmentService.js';

function buildInstance({
  bounds,
  group,
  metadata,
  nodes,
  markers,
}: Omit<ContainmentInstance, 'instanceId'>): ContainmentInstance {
  return {
    instanceId: String(metadata?.instance_id || 'inst-1'),
    group,
    metadata,
    bounds,
    nodes,
    markers,
  };
}

describe('CubeContainmentService', () => {
  test('clamps regular nodes to inner bounds', () => {
    const node = { id: 1, pos: [0, 0], size: [10, 10] };
    const metadata = {
      instance_id: 'inst-1',
      bounds: {
        x: 0,
        y: 0,
        w: 50,
        h: 50,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 10 },
      },
    };
    const group = { pos: [0, 0], size: [50, 50], properties: { sugarcubes: metadata } };
    const instance = buildInstance({
      bounds: metadata.bounds,
      group,
      metadata,
      nodes: [node],
      markers: [],
    });
    const index = {
      instanceByNodeId: new Map([['1', instance]]),
      instanceByMarkerId: new Map(),
      instances: [instance],
    };

    const service = new CubeContainmentService();
    const result = service.enforceForNodes({ nodes: [node], index });

    expect(node.pos).toEqual([2, 12]);
    expect(result.clamped).toBe(1);
    expect(result.expanded).toBe(0);
  });

  test('expands bounds when markers move outside inner bounds', () => {
    const marker = {
      id: 2,
      type: 'SugarCubes.CubeInput',
      pos: [100, 20],
      size: [10, 10],
    };
    const metadata = {
      instance_id: 'inst-2',
      bounds: {
        x: 0,
        y: 0,
        w: 50,
        h: 50,
        padding: { x: 4, y: 4, top_extra: 4 },
        header: { height: 12 },
      },
    };
    const group = { pos: [0, 0], size: [50, 50], properties: { sugarcubes: metadata } };
    const instance = buildInstance({
      bounds: metadata.bounds,
      group,
      metadata,
      nodes: [],
      markers: [marker],
    });
    const index = {
      instanceByNodeId: new Map(),
      instanceByMarkerId: new Map([['2', instance]]),
      instances: [instance],
    };

    const service = new CubeContainmentService();
    const result = service.enforceForNodes({ nodes: [marker], index });

    expect(result.expanded).toBe(1);
    expect(metadata.bounds).toEqual({
      x: 0,
      y: 0,
      w: 112,
      h: 50,
      padding: { x: 4, y: 4, top_extra: 4 },
      header: { height: 12 },
    });
    expect(group.pos).toEqual([0, 0]);
    expect(group.size).toEqual([112, 50]);
  });

  test('clamps using live node position when getBounding is stale', () => {
    const node = {
      id: 3,
      type: 'KSampler',
      pos: new Float64Array([100, 100]),
      size: new Float64Array([10, 10]),
      getBounding: () => [2, 12, 10, 10],
    };
    const metadata = {
      instance_id: 'inst-3',
      bounds: {
        x: 0,
        y: 0,
        w: 50,
        h: 50,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 10 },
      },
    };
    const group = { pos: [0, 0], size: [50, 50], properties: { sugarcubes: metadata } };
    const instance = buildInstance({
      bounds: metadata.bounds,
      group,
      metadata,
      nodes: [node],
      markers: [],
    });
    const index = {
      instanceByNodeId: new Map([['3', instance]]),
      instanceByMarkerId: new Map(),
      instances: [instance],
    };

    const service = new CubeContainmentService();
    const result = service.enforceForNodes({ nodes: [node], index });

    expect([Number(node.pos[0]), Number(node.pos[1])]).toEqual([38, 38]);
    expect(result.clamped).toBe(1);
    expect(result.expanded).toBe(0);
  });

  test('clamps collapsed nodes using visual bounds instead of expanded size', () => {
    const node = {
      id: 4,
      type: 'KSampler',
      flags: { collapsed: true },
      pos: new Float64Array([100, 100]),
      size: new Float64Array([300, 300]),
      getBounding: () => [100, 70, 80, 30],
    };
    const metadata = {
      instance_id: 'inst-4',
      bounds: {
        x: 0,
        y: 0,
        w: 120,
        h: 80,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 10 },
      },
    };
    const group = { pos: [0, 0], size: [120, 80], properties: { sugarcubes: metadata } };
    const instance = buildInstance({
      bounds: metadata.bounds,
      group,
      metadata,
      nodes: [node],
      markers: [],
    });
    const index = {
      instanceByNodeId: new Map([['4', instance]]),
      instanceByMarkerId: new Map(),
      instances: [instance],
    };

    const service = new CubeContainmentService();
    const result = service.enforceForNodes({ nodes: [node], index });

    expect([Number(node.pos[0]), Number(node.pos[1])]).toEqual([38, 78]);
    expect(result.clamped).toBe(1);
    expect(result.expanded).toBe(0);
  });

  test('clamps collapsed nodes even when getBounding position is stale', () => {
    const node = {
      id: 5,
      type: 'KSampler',
      flags: { collapsed: true },
      pos: new Float64Array([400, 300]),
      size: new Float64Array([300, 300]),
      _collapsed_width: 80,
      getBounding: () => [100, 70, 80, 30],
    };
    const metadata = {
      instance_id: 'inst-5',
      bounds: {
        x: 0,
        y: 0,
        w: 120,
        h: 80,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 10 },
      },
    };
    const group = { pos: [0, 0], size: [120, 80], properties: { sugarcubes: metadata } };
    const instance = buildInstance({
      bounds: metadata.bounds,
      group,
      metadata,
      nodes: [node],
      markers: [],
    });
    const index = {
      instanceByNodeId: new Map([['5', instance]]),
      instanceByMarkerId: new Map(),
      instances: [instance],
    };

    const service = new CubeContainmentService();
    const result = service.enforceForNodes({ nodes: [node], index });

    expect([Number(node.pos[0]), Number(node.pos[1])]).toEqual([38, 78]);
    expect(result.clamped).toBe(1);
    expect(result.expanded).toBe(0);
  });
});
