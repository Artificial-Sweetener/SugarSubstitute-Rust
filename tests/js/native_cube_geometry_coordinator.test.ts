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
/** Verify native Cube editors recover renderer-safe authored node relations. */

import { expect, jest, test } from '@jest/globals';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { NativeCubeGeometryCoordinator } from '../../frontend/comfyui/ui/cube/geometry/NativeCubeGeometryCoordinator.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('moves later native cards after mounted Vue geometry exceeds authored size', () => {
  const { cube, graph, first, second } = createFixture();
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  let currentGraph: object | null = null;
  const graphListeners = new Set<() => void>();
  const scheduled: Array<() => void> = [];
  const coordinator = new NativeCubeGeometryCoordinator({
    document,
    nodes: catalog,
    graphChanges: {
      subscribe(listener) {
        graphListeners.add(listener);
        return () => graphListeners.delete(listener);
      },
    },
    getCurrentGraph: () => currentGraph,
    getRenderer: () => 'vue',
    scheduler: {
      schedule(callback) {
        scheduled.push(callback);
        return callback;
      },
      cancel: () => undefined,
    },
  });
  mountCard(first.id, 100, 180);
  mountCard(second.id, 100, 100);

  currentGraph = graph;
  for (const listener of graphListeners) listener();

  expect(first.pos).toEqual([0, 30]);
  expect(second.pos).toEqual([0, 240]);
  expect(graph.inputNode.pos).toEqual([-100, 60]);
  expect(graph.outputNode.pos).toEqual([140, 270]);
  expect(graph.inputNode.bounding?.slice(0, 2)).toEqual([-100, 60]);
  expect(graph.outputNode.bounding?.slice(0, 2)).toEqual([140, 270]);
  expect(graph.inputNode.boundingRect?.slice(0, 2)).toEqual([-100, 60]);
  expect(graph.outputNode.boundingRect?.slice(0, 2)).toEqual([140, 270]);
  expect(graph.inputNode.slots?.[0]?.pos).toEqual([-50, 70]);
  expect(graph.outputNode.slots?.[0]?.pos).toEqual([150, 280]);
  expect(graph.setDirtyCanvas).toHaveBeenCalledWith(true, true);
  coordinator.dispose();
  document.querySelectorAll('[data-node-id]').forEach((element) => element.remove());
});

test('repairs persisted boundary slot anchors when boundary positions are already correct', () => {
  const { cube, graph, first, second } = createFixture();
  const inputBoundary = graph.inputNode;
  const outputBoundary = graph.outputNode;
  for (const target of [inputBoundary.bounding, inputBoundary.boundingRect, inputBoundary.pos]) {
    if (!target) continue;
    target[0] = -100;
    target[1] = 60;
  }
  for (const target of [outputBoundary.bounding, outputBoundary.boundingRect, outputBoundary.pos]) {
    if (!target) continue;
    target[0] = 140;
    target[1] = 190;
  }
  inputBoundary.slots![0]!.pos = [900, -700];
  outputBoundary.slots![0]!.pos = [-800, 600];
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  mountCard(first.id, 100, 100);
  mountCard(second.id, 100, 100);

  const coordinator = new NativeCubeGeometryCoordinator({
    document,
    nodes: catalog,
    graphChanges: { subscribe: () => () => undefined },
    getCurrentGraph: () => graph,
    getRenderer: () => 'vue',
    scheduler: {
      schedule: () => null,
      cancel: () => undefined,
    },
  });

  expect(first.pos).toEqual([0, 30]);
  expect(second.pos).toEqual([0, 160]);
  expect(inputBoundary.slots?.[0]?.pos).toEqual([-50, 70]);
  expect(outputBoundary.slots?.[0]?.pos).toEqual([150, 200]);
  expect(graph.setDirtyCanvas).toHaveBeenCalledWith(true, true);
  coordinator.dispose();
  document.querySelectorAll('[data-node-id]').forEach((element) => element.remove());
});

test('waits for native Vue cards to mount before solving their positions', () => {
  const { cube, graph, first, second } = createFixture();
  if (graph.extra) delete graph.extra.sugarcubes_boundary_layout;
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  let currentGraph: object | null = graph;
  const scheduled: Array<() => void> = [];
  const coordinator = new NativeCubeGeometryCoordinator({
    document,
    nodes: catalog,
    graphChanges: { subscribe: () => () => undefined },
    getCurrentGraph: () => currentGraph,
    getRenderer: () => 'vue',
    scheduler: {
      schedule(callback) {
        scheduled.push(callback);
        return callback;
      },
      cancel: () => undefined,
    },
  });

  expect(second.pos).toEqual([0, 160]);
  scheduled.shift()?.();
  expect(second.pos).toEqual([0, 160]);

  mountCard(first.id, 100, 180);
  mountCard(second.id, 100, 100);
  scheduled.shift()?.();

  expect(second.pos).toEqual([0, 240]);
  expect(graph.inputNode.pos).toEqual([-100, 165]);
  expect(graph.outputNode.pos).toEqual([140, 165]);
  coordinator.dispose();
  currentGraph = null;
  document.querySelectorAll('[data-node-id]').forEach((element) => element.remove());
});

/** Build one two-card Cube with a preserved vertical gap in authored coordinates. */
function createFixture(): {
  cube: CubeNode;
  graph: NativeCubeSubgraph & { setDirtyCanvas: jest.Mock };
  first: CubeNode['subgraph']['_nodes'][number];
  second: CubeNode['subgraph']['_nodes'][number];
} {
  const first = {
    id: 'first-node',
    type: 'First',
    title: 'First',
    pos: [0, 30],
    size: [100, 100],
    properties: { sugarcubes_symbol: 'first' },
    inputs: [],
    outputs: [],
    connect() {},
  };
  const second = {
    id: 'second-node',
    type: 'Second',
    title: 'Second',
    pos: [0, 160],
    size: [100, 100],
    properties: { sugarcubes_symbol: 'second' },
    inputs: [],
    outputs: [],
    connect() {},
  };
  const graph = {
    id: 'cube-definition',
    name: 'Cube',
    _nodes: [first, second],
    inputs: [],
    outputs: [],
    inputNode: createBoundaryFixture('input'),
    outputNode: createBoundaryFixture('output'),
    add() {},
    remove() {},
    addInput() {
      throw new Error('unused');
    },
    addOutput() {
      throw new Error('unused');
    },
    configure() {},
    extra: {
      sugarcubes_authored_layout: {
        schema: 1,
        origin: [0, 0],
        entries: {
          first: { x: 0, y: 0, w: 100, h: 100, collapsed: false, title: 'First' },
          second: { x: 0, y: 130, w: 100, h: 100, collapsed: false, title: 'Second' },
        },
        group: null,
      },
      sugarcubes_boundary_layout: {
        schema: 1,
        inputSymbols: ['first'],
        outputSymbols: ['second'],
      },
    },
    setDirtyCanvas: jest.fn(),
  } as unknown as NativeCubeSubgraph & { setDirtyCanvas: jest.Mock };
  const cube = {
    id: 'cube-instance',
    type: graph.id,
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-instance' },
    },
    inputs: [],
    outputs: [],
    subgraph: graph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  } as CubeNode;
  return { cube, graph, first, second };
}

/** Model Comfy's host-owned IO arrangement contract for one boundary. */
function createBoundaryFixture(kind: 'input' | 'output'): {
  bounding: number[];
  boundingRect: number[];
  emptySlot: { name: string; pos: number[]; boundingRect: number[] };
  pos: number[];
  size: number[];
  slots: Array<{ pos: number[]; boundingRect: number[] }>;
  arrange(): void;
} {
  const boundingRect = [0, 0, 60, 40];
  const pos = [0, 0];
  const slot = { pos: [0, 0], boundingRect: [0, 0, 20, 20] };
  const emptySlot = { name: '', pos: [0, 0], boundingRect: [0, 0, 20, 20] };
  return {
    bounding: [0, 0, 60, 40],
    boundingRect,
    emptySlot,
    pos,
    size: [60, 40],
    slots: [slot],
    arrange() {
      const anchorX = kind === 'input' ? boundingRect[0] + 50 : boundingRect[0] + 10;
      slot.pos = [anchorX, boundingRect[1] + 10];
      slot.boundingRect = [anchorX - 10, boundingRect[1], 20, 20];
      emptySlot.pos = [anchorX, boundingRect[1] + 30];
      emptySlot.boundingRect = [anchorX - 10, boundingRect[1] + 20, 20, 20];
    },
  };
}

/** Mount one measurable Nodes 2 card for an internal graph node. */
function mountCard(id: string | number, width: number, height: number): void {
  const element = document.createElement('article');
  element.dataset.nodeId = String(id);
  Object.defineProperties(element, {
    offsetWidth: { value: width },
    offsetHeight: { value: height },
  });
  document.body.append(element);
}
