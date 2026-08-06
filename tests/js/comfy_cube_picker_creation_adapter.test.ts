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
/** Verify the picker creation seam claims only Sugar's reserved namespace. */

import { describe, expect, jest, test } from '@jest/globals';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import {
  ComfyCubePickerCreationAdapter,
  type CubePickerLiteGraphHost,
} from '../../frontend/comfyui/ui/picker/ComfyCubePickerCreationAdapter.js';
import type { CubePickerPlacementAdapter } from '../../frontend/comfyui/ui/picker/CubePickerPlacementAdapter.js';

const cubeType = `SugarCubes.Cube.${'a'.repeat(64)}`;

describe('ComfyCubePickerCreationAdapter', () => {
  test('delegates ordinary types exactly and returns the final detached Cube for Sugar types', () => {
    const ordinaryResult = { type: 'ordinary' };
    const original = jest.fn(() => ordinaryResult);
    const liteGraph: CubePickerLiteGraphHost = { createNode: original };
    const cube = cubeNode();
    const create = jest.fn(() => cube);
    const reportError = jest.fn();
    const adapter = new ComfyCubePickerCreationAdapter({
      getLiteGraph: () => liteGraph,
      placement: { create } as unknown as CubePickerPlacementAdapter,
      reportError,
    });
    adapter.install();
    const installed = liteGraph.createNode;
    adapter.install();

    const ordinaryOptions = { test: true };
    expect(liteGraph.createNode('ordinary/type', 'Ordinary', ordinaryOptions)).toBe(ordinaryResult);
    expect(original).toHaveBeenCalledWith('ordinary/type', 'Ordinary', ordinaryOptions);
    expect(liteGraph.createNode).toBe(installed);

    const created = liteGraph.createNode(cubeType, 'Picker title', {
      pos: [120, 240],
      customHostOption: 'preserved',
    });
    expect(created).toBe(cube);
    expect(create).toHaveBeenCalledWith(cubeType, [120, 240]);
    expect(cube.title).toBe('Picker title');
    expect(cube.pos).toEqual([120, 240]);
    expect(cube.customHostOption).toBe('preserved');
    expect(reportError).not.toHaveBeenCalled();
  });

  test('reports claimed-type failures and never falls back to a malformed generic node', () => {
    const original = jest.fn(() => ({ type: 'generic' }));
    const liteGraph: CubePickerLiteGraphHost = { createNode: original };
    const reportError = jest.fn();
    const adapter = new ComfyCubePickerCreationAdapter({
      getLiteGraph: () => liteGraph,
      placement: {
        create: () => {
          throw new Error('Prepared Cube unavailable');
        },
      } as unknown as CubePickerPlacementAdapter,
      reportError,
    });
    adapter.install();

    expect(() => liteGraph.createNode(cubeType)).toThrow('Prepared Cube unavailable');
    expect(original).not.toHaveBeenCalled();
    expect(reportError).toHaveBeenCalledWith(
      'SugarCube placement failed',
      'Prepared Cube unavailable',
    );
  });

  test('hands an existing Sugar wrapper to the newest extension module instance', () => {
    const original = jest.fn();
    const liteGraph: CubePickerLiteGraphHost = { createNode: original };
    const firstCreate = jest.fn(() => cubeNode());
    const first = new ComfyCubePickerCreationAdapter({
      getLiteGraph: () => liteGraph,
      placement: { create: firstCreate } as unknown as CubePickerPlacementAdapter,
      reportError: jest.fn(),
    });
    first.install();
    const installed = liteGraph.createNode;
    const secondCube = cubeNode();
    const secondCreate = jest.fn(() => secondCube);
    const second = new ComfyCubePickerCreationAdapter({
      getLiteGraph: () => liteGraph,
      placement: { create: secondCreate } as unknown as CubePickerPlacementAdapter,
      reportError: jest.fn(),
    });

    second.install();
    expect(liteGraph.createNode).toBe(installed);
    expect(liteGraph.createNode(cubeType)).toBe(secondCube);
    expect(firstCreate).not.toHaveBeenCalled();
    expect(secondCreate).toHaveBeenCalledTimes(1);

    first.dispose();
    expect(liteGraph.createNode).toBe(installed);
    second.dispose();
    expect(liteGraph.createNode).toBe(original);
  });

  test('restores only its own wrapper and preserves a later extension wrapper', () => {
    const original = jest.fn();
    const liteGraph: CubePickerLiteGraphHost = { createNode: original };
    const adapter = new ComfyCubePickerCreationAdapter({
      getLiteGraph: () => liteGraph,
      placement: { create: () => cubeNode() } as unknown as CubePickerPlacementAdapter,
      reportError: jest.fn(),
    });
    adapter.install();
    const sugarWrapper = liteGraph.createNode;
    const laterWrapper = jest.fn((...args: Parameters<typeof sugarWrapper>) =>
      sugarWrapper(...args),
    );
    liteGraph.createNode = laterWrapper;

    adapter.dispose();

    expect(liteGraph.createNode).toBe(laterWrapper);
  });
});

/** Build the minimum detached Cube node surface used by host options. */
function cubeNode(): CubeNode {
  return {
    id: 'instance',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [320, 180],
    properties: {},
    inputs: [],
    outputs: [],
    subgraph: { id: 'definition', name: 'Cube', _nodes: [], inputs: [], outputs: [] },
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  } as unknown as CubeNode;
}
