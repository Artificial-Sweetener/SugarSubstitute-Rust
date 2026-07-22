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
import { describe, expect, jest, test } from '@jest/globals';
import { RendererGeometryCoordinator } from '../../frontend/comfyui/ui/geometry/RendererGeometryCoordinator.js';
import type { ComfyGraph, ComfyGroup, ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

function createFixture(): {
  graph: ComfyGraph;
  group: ComfyGroup;
  nodes: ComfyNode[];
} {
  const nodes: ComfyNode[] = [
    {
      id: 1,
      pos: [0, 0],
      size: [140, 80],
      properties: { sugarcubes_symbol: 'first' },
    },
    {
      id: 2,
      pos: [160, 0],
      size: [140, 80],
      properties: { sugarcubes_symbol: 'second' },
    },
  ];
  const group: ComfyGroup = {
    pos: [-10, -40],
    size: [320, 130],
    properties: {
      sugarcubes: {
        instance_id: 'instance-1',
        nodes: ['1', '2'],
        markers: { inputs: [], outputs: [] },
        bounds: { x: -10, y: -40, w: 320, h: 130 },
        authored_layout: {
          schema: 1,
          origin: [0, 0],
          entries: {
            first: { x: 0, y: 0, w: 140, h: 80, collapsed: false, title: 'First' },
            second: { x: 160, y: 0, w: 140, h: 80, collapsed: false, title: 'Second' },
          },
          group: { x: -10, y: -40, w: 320, h: 130 },
        },
      },
    },
  };
  return { graph: { _nodes: nodes, _groups: [group], setDirtyCanvas: jest.fn() }, group, nodes };
}

function createScheduler(frames: FrameRequestCallback[]) {
  return {
    raf: (callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    },
    timeout: () => null,
    clearTimeout: () => undefined,
  };
}

function flushFrames(frames: FrameRequestCallback[]): void {
  while (frames.length) frames.shift()?.(0);
}

describe('renderer geometry coordinator', () => {
  test('solves renderer growth from the immutable authored baseline', () => {
    const { graph, group, nodes } = createFixture();
    const liteGraph = { vueNodesMode: false } as LiteGraphHost;
    const frames: FrameRequestCallback[] = [];
    const onStabilized = jest.fn();
    const setDirty = jest.fn();
    const coordinator = new RendererGeometryCoordinator({
      adapter: {
        getGraph: () => graph,
        getCanvas: () => ({ setDirty }),
        getLiteGraph: () => liteGraph,
      },
      scheduler: createScheduler(frames),
      onStabilized,
    });

    liteGraph.vueNodesMode = true;
    coordinator.checkRenderer();
    expect(nodes[0]?.size).toEqual([225, 80]);
    flushFrames(frames);

    expect(nodes[1]?.pos).toEqual([245, 30]);
    expect(group.pos).toEqual([-10, -40]);
    expect(group.size).toEqual([490, 160]);
    expect(onStabilized).toHaveBeenCalledWith(graph);
    expect(setDirty).toHaveBeenCalledWith(true, true);
  });

  test('uses post-mount DOM measurements instead of layout-store sizes', () => {
    const { graph, nodes } = createFixture();
    const frames: FrameRequestCallback[] = [];
    const first = document.createElement('article');
    first.dataset.nodeId = '1';
    Object.defineProperties(first, {
      offsetWidth: { value: 300 },
      offsetHeight: { value: 80 },
    });
    const second = document.createElement('article');
    second.dataset.nodeId = '2';
    Object.defineProperties(second, {
      offsetWidth: { value: 225 },
      offsetHeight: { value: 80 },
    });
    document.body.append(first, second);
    const coordinator = new RendererGeometryCoordinator({
      adapter: {
        getGraph: () => graph,
        getLiteGraph: () => ({ vueNodesMode: true }) as LiteGraphHost,
        getDocument: () => document,
      },
      scheduler: createScheduler(frames),
    });

    coordinator.scheduleStabilization(nodes);
    flushFrames(frames);

    expect(nodes[1]?.pos).toEqual([320, 30]);
    first.remove();
    second.remove();
  });

  test('repeated renderer switching never accumulates transforms', () => {
    const { graph, group, nodes } = createFixture();
    const liteGraph = { vueNodesMode: false } as LiteGraphHost;
    const frames: FrameRequestCallback[] = [];
    const coordinator = new RendererGeometryCoordinator({
      adapter: { getGraph: () => graph, getLiteGraph: () => liteGraph },
      scheduler: createScheduler(frames),
    });

    liteGraph.vueNodesMode = true;
    coordinator.checkRenderer();
    flushFrames(frames);
    expect(nodes.map((node) => node.pos)).toEqual([
      [0, 30],
      [245, 30],
    ]);

    liteGraph.vueNodesMode = false;
    coordinator.checkRenderer();
    flushFrames(frames);
    expect(nodes.map((node) => ({ pos: node.pos, size: node.size }))).toEqual([
      { pos: [0, 0], size: [140, 80] },
      { pos: [160, 0], size: [140, 80] },
    ]);
    expect(group.size).toEqual([320, 130]);

    liteGraph.vueNodesMode = true;
    coordinator.checkRenderer();
    flushFrames(frames);
    expect(nodes.map((node) => node.pos)).toEqual([
      [0, 30],
      [245, 30],
    ]);
  });

  test('preserves a moved cube anchor across renderer switching', () => {
    const { graph, group, nodes } = createFixture();
    const liteGraph = { vueNodesMode: false } as LiteGraphHost;
    const frames: FrameRequestCallback[] = [];
    const coordinator = new RendererGeometryCoordinator({
      adapter: { getGraph: () => graph, getLiteGraph: () => liteGraph },
      scheduler: createScheduler(frames),
    });
    group.pos = [390, 260];
    nodes[0]!.pos = [400, 300];
    nodes[1]!.pos = [560, 300];

    liteGraph.vueNodesMode = true;
    coordinator.checkRenderer();
    flushFrames(frames);

    expect(group.pos).toEqual([390, 260]);
    expect(nodes.map((node) => node.pos)).toEqual([
      [400, 330],
      [645, 330],
    ]);
  });

  test('waits for late Nodes 2 cards before measuring their geometry', () => {
    const { graph, nodes } = createFixture();
    const timeouts: Array<() => void> = [];
    const coordinator = new RendererGeometryCoordinator({
      adapter: {
        getGraph: () => graph,
        getNodeRenderer: () => 'vue',
        getDocument: () => document,
      },
      scheduler: {
        raf: () => null,
        timeout: (callback) => {
          timeouts.push(callback);
          return timeouts.length;
        },
        clearTimeout: () => undefined,
      },
    });

    coordinator.scheduleStabilization(nodes);
    timeouts.shift()?.();
    expect(nodes[1]?.pos).toEqual([160, 0]);

    for (const [index, width] of [300, 225].entries()) {
      const element = document.createElement('article');
      element.dataset.nodeId = String(index + 1);
      Object.defineProperties(element, {
        offsetWidth: { value: width },
        offsetHeight: { value: 80 },
      });
      document.body.append(element);
    }
    timeouts.shift()?.();

    expect(nodes[1]?.pos).toEqual([320, 30]);
    document.querySelectorAll('[data-node-id]').forEach((element) => element.remove());
  });

  test('treats float-vector storage precision as stable geometry', () => {
    const { graph, group, nodes } = createFixture();
    const metadata = group.properties?.sugarcubes as {
      authored_layout: { origin: [number, number] };
    };
    metadata.authored_layout.origin = [0.1, 0.1];
    group.pos = new Float32Array([-9.9, -39.9]);
    group.size = new Float32Array([320, 130]);
    nodes[0]!.pos = [0.1, 0.1];
    nodes[1]!.pos = [160.1, 0.1];
    const frames: FrameRequestCallback[] = [];
    const onStabilized = jest.fn();
    const coordinator = new RendererGeometryCoordinator({
      adapter: {
        getGraph: () => graph,
        getLiteGraph: () => ({ vueNodesMode: false }) as LiteGraphHost,
      },
      scheduler: createScheduler(frames),
      onStabilized,
    });

    coordinator.scheduleStabilization(nodes);
    flushFrames(frames);

    expect(onStabilized).not.toHaveBeenCalled();
  });
});
