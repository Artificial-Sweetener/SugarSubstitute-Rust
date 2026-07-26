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
import { CubeEditorNavigationPresenter } from '../../frontend/comfyui/ui/surface/CubeEditorNavigationPresenter.js';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('returns through Cube context instead of skipping from nested graph to root', () => {
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
  const captureView = jest.fn(() => ({ scale: 0.75, offset: [80, 120] as const }));
  const restoreView = jest.fn();
  const selected = { id: 'selected-root-node' };
  const restoreSelection = jest.fn();
  const presenter = new CubeEditorNavigationPresenter({
    document,
    canvas: {
      setGraph,
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
  currentGraph = { id: 'nested', name: 'Restored nested graph' };
  presenter.refresh();

  const bar = document.querySelector('.sugarcubes-cube-editor-navigation');
  expect(bar?.textContent).toContain('Cube Editor');
  expect(bar?.textContent).toContain('Detailer / Nested Detail');
  const buttons = [...(bar?.querySelectorAll('button') ?? [])];
  expect(buttons.map((button) => button.textContent)).toEqual(['Back', 'Exit Cube']);

  buttons[0]?.click();
  expect(setGraph).toHaveBeenLastCalledWith(cubeGraph);
  presenter.refresh();
  expect(bar?.textContent).toContain('Detailer');
  expect(bar?.textContent).not.toContain('Nested Detail');

  const exit = [...(bar?.querySelectorAll('button') ?? [])].find(
    (button) => button.textContent === 'Exit Cube',
  );
  exit?.click();
  expect(setGraph).toHaveBeenLastCalledWith(rootGraph);
  expect(restoreView).toHaveBeenLastCalledWith({
    scale: 0.75,
    offset: [80, 120],
  });
  presenter.refresh();
  expect(restoreView).toHaveBeenCalledTimes(2);
  expect(restoreSelection).toHaveBeenCalledTimes(2);
  expect(restoreSelection).toHaveBeenLastCalledWith([selected]);
  presenter.dispose();
  jest.useRealTimers();
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
    document,
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

  const back = [
    ...document.querySelectorAll<HTMLButtonElement>('.sugarcubes-cube-editor-navigation button'),
  ].find((button) => button.textContent === 'Back');
  expect(back?.closest('main')).toBeNull();
  back?.click();

  expect(setGraph).toHaveBeenLastCalledWith(liveCubeGraph);
  presenter.refresh();
  expect(document.querySelector('.sugarcubes-cube-editor-navigation')?.textContent).toContain(
    'Detailer',
  );
  expect(document.querySelector('.sugarcubes-cube-editor-navigation')?.textContent).not.toContain(
    'Nested Detail',
  );
  presenter.dispose();
  jest.useRealTimers();
});

test('contains Cube navigation actions so the host cannot apply a second back transition', () => {
  jest.useFakeTimers();
  document.body.replaceChildren();
  const main = document.createElement('main');
  const navigation = document.createElement('nav');
  navigation.setAttribute('aria-label', 'Graph navigation');
  main.append(navigation);
  document.body.append(main);
  const rootGraph = { id: 'root' };
  const nested = { id: 'nested', name: 'KSampler', _nodes: [] };
  const cubeGraph = {
    id: 'cube-definition',
    name: 'SDXL/Text to Image',
    _nodes: [{ isSubgraphNode: () => true, subgraph: nested }],
  } as unknown as NativeCubeSubgraph;
  const nodes = new CubeNodeCatalog();
  const cube = cubeNode('container', 'SDXL/Text to Image', cubeGraph);
  nodes.add(cube);
  let currentGraph: object = rootGraph;
  const setGraph = jest.fn((graph: object) => {
    currentGraph = graph;
  });
  const presenter = new CubeEditorNavigationPresenter({
    document,
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
  currentGraph = nested;
  jest.advanceTimersByTime(100);
  main.addEventListener('click', () => setGraph(rootGraph));

  const back = [
    ...document.querySelectorAll<HTMLButtonElement>('.sugarcubes-cube-editor-navigation button'),
  ].find((button) => button.textContent === 'Back');
  back?.click();

  expect(setGraph).toHaveBeenLastCalledWith(cubeGraph);
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
    document,
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
