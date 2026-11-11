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
/** Verify distinct Cube editor navigation across nested native subgraphs. */

import { jest } from '@jest/globals';
import { CubeEditorNavigationPresenter } from '../../../frontend/comfyui/ui/surface/CubeEditorNavigationPresenter.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { EMPTY_CUBE_BOUNDARY_VIEWPORT_BOUNDS } from '../../../frontend/comfyui/ui/cube/geometry/NativeCubeBoundaryLayout.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('keeps Cube session context without injecting a second editor bar', () => {
  jest.useFakeTimers();
  document.body.replaceChildren();
  const main = document.createElement('main');
  const navigation = document.createElement('nav');
  navigation.setAttribute('aria-label', 'Graph navigation');
  main.append(navigation);
  document.body.append(main);
  const rootGraph = { id: 'root' };
  const nested = { id: 'nested', name: 'Nested Detail', _nodes: [] };
  const cubeGraph = {
    id: 'cube-definition',
    name: 'Cube: Detailer',
    _nodes: [
      {
        isSubgraphNode: () => true,
        subgraph: nested,
      },
    ],
  } as unknown as NativeCubeSubgraph;
  const nodes = new CubeNodeCatalog();
  const cube = cubeNode('container', 'Detailer', cubeGraph);
  nodes.add(cube);
  let currentGraph: object = rootGraph;
  const setGraph = jest.fn((graph: object) => {
    currentGraph = graph;
  });
  const openSubgraph = jest.fn((graph: object) => {
    currentGraph = graph;
  });
  const captureView = jest.fn(() => ({ scale: 0.75, offset: [80, 120] as const }));
  const restoreView = jest.fn();
  const selected = { id: 'selected-root-node' };
  const restoreSelection = jest.fn();
  const presenter = new CubeEditorNavigationPresenter({
    canvas: {
      setGraph,
      openSubgraph,
      captureView,
      restoreView,
      captureSelection: () => [selected],
      restoreSelection,
    },
    rootGraph,
    getCurrentGraph: () => currentGraph,
    nodes,
  });
  presenter.open(cube);
  expect(openSubgraph).toHaveBeenCalledWith(cubeGraph, cube);
  expect(setGraph).not.toHaveBeenCalled();
  currentGraph = { id: 'nested', name: 'Restored nested graph' };
  presenter.refresh();
  expect(document.querySelector('.sugarcubes-cube-editor-navigation')).toBeNull();
  expect(main.contains(navigation)).toBe(true);

  currentGraph = cubeGraph;
  presenter.refresh();
  currentGraph = rootGraph;
  presenter.refresh();
  expect(restoreView).toHaveBeenLastCalledWith({
    scale: 0.75,
    offset: [80, 120],
  });
  expect(restoreView).toHaveBeenCalledTimes(1);
  expect(restoreSelection).toHaveBeenCalledTimes(1);
  expect(restoreSelection).toHaveBeenLastCalledWith([selected]);
  presenter.dispose();
  jest.useRealTimers();
});

test('frames empty Cube boundaries after Comfy completes its graph transition', () => {
  document.body.replaceChildren();
  const rootGraph = { id: 'root' };
  const cubeGraph = {
    id: 'cube-definition',
    name: 'Untitled Cube',
    _nodes: [],
    inputNode: { emptySlot: { name: 'Add Input' } },
    outputNode: { emptySlot: { name: 'Add Output' } },
  } as unknown as NativeCubeSubgraph;
  const nodes = new CubeNodeCatalog();
  const cube = cubeNode('draft', 'Untitled Cube', cubeGraph);
  nodes.add(cube);
  let currentGraph: object = rootGraph;
  const focusBounds = jest.fn(() => true);
  const frames: Array<() => void> = [];
  const presenter = new CubeEditorNavigationPresenter({
    canvas: {
      setGraph(graph) {
        currentGraph = graph;
      },
      focusBounds,
      captureView: () => null,
      restoreView() {},
      captureSelection: () => [],
      restoreSelection() {},
    },
    rootGraph,
    getCurrentGraph: () => currentGraph,
    nodes,
    scheduleFrame(callback) {
      frames.push(callback);
    },
  });

  presenter.prepare(cube);
  frames.shift()?.();
  expect(focusBounds).not.toHaveBeenCalled();

  currentGraph = cubeGraph;
  presenter.refresh();
  frames.shift()?.();

  expect(focusBounds).toHaveBeenCalledWith(EMPTY_CUBE_BOUNDARY_VIEWPORT_BOUNDS);
  expect(cubeGraph.inputNode.emptySlot?.name).toBe('Add Input');
  expect(cubeGraph.outputNode.emptySlot?.name).toBe('Add Output');
  presenter.dispose();
});

test('frames an empty Cube entered through Comfy native navigation without a footer hook', () => {
  document.body.replaceChildren();
  const rootGraph = { id: 'root' };
  const cubeGraph = {
    id: 'cube-definition',
    name: 'Untitled Cube',
    _nodes: [],
  } as unknown as NativeCubeSubgraph;
  const nodes = new CubeNodeCatalog();
  const cube = cubeNode('empty-cube', 'Untitled Cube', cubeGraph);
  nodes.add(cube);
  const frames: Array<() => void> = [];
  const focusBounds = jest.fn(() => true);
  const presenter = new CubeEditorNavigationPresenter({
    canvas: {
      setGraph() {},
      focusBounds,
      captureView: () => null,
      restoreView() {},
      captureSelection: () => [],
      restoreSelection() {},
    },
    rootGraph,
    getCurrentGraph: () => cubeGraph,
    nodes,
    scheduleFrame(callback) {
      frames.push(callback);
    },
  });

  frames.shift()?.();

  expect(focusBounds).toHaveBeenCalledWith(EMPTY_CUBE_BOUNDARY_VIEWPORT_BOUNDS);
  presenter.dispose();
});

test('returns to the live Cube graph object after Comfy replaces definition instances', () => {
  jest.useFakeTimers();
  document.body.replaceChildren();
  const main = document.createElement('main');
  const navigation = document.createElement('nav');
  navigation.setAttribute('aria-label', 'Graph navigation');
  main.append(navigation);
  document.body.append(main);
  const rootGraph = { id: 'root' };
  const persistedCubeGraph = {
    id: 'cube-definition',
    name: 'Cube: Detailer',
    _nodes: [],
  } as unknown as NativeCubeSubgraph;
  const liveCubeGraph = {
    id: 'cube-definition',
    name: 'Cube: Detailer',
    _nodes: [],
  };
  const nested = { id: 'nested', name: 'Nested Detail', _nodes: [] };
  const nodes = new CubeNodeCatalog();
  const cube = cubeNode('container', 'Detailer', persistedCubeGraph);
  nodes.add(cube);
  let currentGraph: object = rootGraph;
  const setGraph = jest.fn((graph: object) => {
    currentGraph = graph;
  });
  const presenter = new CubeEditorNavigationPresenter({
    canvas: {
      setGraph,
      captureView: () => ({ scale: 1, offset: [0, 0] }),
      restoreView: jest.fn(),
      captureSelection: () => [],
      restoreSelection: jest.fn(),
    },
    rootGraph,
    getCurrentGraph: () => currentGraph,
    nodes,
  });

  presenter.open(cube);
  currentGraph = liveCubeGraph;
  presenter.refresh();
  currentGraph = nested;
  presenter.refresh();
  expect(document.querySelector('.sugarcubes-cube-editor-navigation')).toBeNull();

  currentGraph = liveCubeGraph;
  presenter.refresh();
  expect(document.querySelector('.sugarcubes-cube-editor-navigation')).toBeNull();
  presenter.dispose();
  jest.useRealTimers();
});

test('does not traverse Cube definitions while the root graph is idle', () => {
  jest.useFakeTimers();
  document.body.replaceChildren();
  const rootGraph = { id: 'root' };
  const isSubgraphNode = jest.fn(() => true);
  const cubeGraph = {
    id: 'cube-definition',
    name: 'Idle Cube',
    _nodes: [{ isSubgraphNode, subgraph: { id: 'nested', _nodes: [] } }],
  } as unknown as NativeCubeSubgraph;
  const nodes = new CubeNodeCatalog();
  nodes.add(cubeNode('container', 'Idle Cube', cubeGraph));
  const presenter = new CubeEditorNavigationPresenter({
    canvas: {
      setGraph() {},
      captureView: () => null,
      restoreView() {},
      captureSelection: () => [],
      restoreSelection() {},
    },
    rootGraph,
    getCurrentGraph: () => rootGraph,
    nodes,
  });
  isSubgraphNode.mockClear();

  jest.advanceTimersByTime(1_000);

  expect(isSubgraphNode).not.toHaveBeenCalled();
  presenter.dispose();
  jest.useRealTimers();
});

/** Build one real Cube node around a native subgraph definition. */
function cubeNode(id: string, title: string, subgraph: NativeCubeSubgraph): CubeNode {
  return {
    id,
    type: subgraph.id,
    title,
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: id, cube_id: `${id}.cube` },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
