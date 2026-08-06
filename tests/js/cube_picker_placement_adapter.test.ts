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
/** Verify picker placement delegates to validated detached construction. */

import { describe, expect, jest, test } from '@jest/globals';
import type { ComfyCubeRuntime } from '../../frontend/comfyui/ui/cube/ComfyCubeRuntime.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubePickerCatalogRegistry } from '../../frontend/comfyui/ui/picker/CubePickerCatalogRegistry.js';
import type { CubePickerDescriptor } from '../../frontend/comfyui/ui/picker/CubePickerDescriptor.js';
import { CubePickerPlacementAdapter } from '../../frontend/comfyui/ui/picker/CubePickerPlacementAdapter.js';

const type = `SugarCubes.Cube.${'a'.repeat(64)}`;

describe('CubePickerPlacementAdapter', () => {
  test('prepares a fresh positioned payload and returns construction without graph insertion', () => {
    const node = { id: 'instance', pos: [120, 240] } as unknown as CubeNode;
    const construct = jest.fn(() => ({
      node,
      subgraph: {},
      warnings: ['construction warning'],
      internalNodeCount: 2,
    }));
    const registerSubgraphs = jest.fn(() => ['registration warning']);
    const runtime = {
      construction: { construct },
      registerSubgraphs,
    } as unknown as ComfyCubeRuntime;
    const cachedPayload = {
      cube: { cube_id: 'local/demo.cube', version: '1.0.0' },
      nodes: [],
      markers: [],
      connections: [],
      subgraphs: [],
      layout: { origin: [0, 0], groups: [] },
    };
    const logger = { warn: jest.fn() };
    const adapter = new CubePickerPlacementAdapter({
      registry: {
        descriptor: () => descriptor(),
        preparedPayload: () => cachedPayload,
      } as unknown as CubePickerCatalogRegistry,
      getRuntime: () => runtime,
      getLiteGraph: () => ({ createNode: jest.fn(), vueNodesMode: true }),
      getNodeRenderer: () => 'vue',
      logger,
    });

    const created = adapter.create(type, [120, 240]);

    expect(created).toBe(node);
    expect(registerSubgraphs).toHaveBeenCalledTimes(1);
    expect(construct).toHaveBeenCalledWith(
      expect.objectContaining({ layout: expect.objectContaining({ origin: [120, 240] }) }),
      { instanceAlias: 'Demo Cube', position: [120, 240] },
    );
    expect(cachedPayload.layout.origin).toEqual([0, 0]);
    expect(logger.warn).toHaveBeenCalledTimes(2);
  });

  test('rejects stale advertised types before mutating the runtime', () => {
    const getRuntime = jest.fn<() => ComfyCubeRuntime>();
    const adapter = new CubePickerPlacementAdapter({
      registry: {
        descriptor: () => null,
        preparedPayload: () => null,
      } as unknown as CubePickerCatalogRegistry,
      getRuntime,
      getLiteGraph: () => ({ createNode: jest.fn() }),
      getNodeRenderer: () => 'litegraph',
      logger: { warn: jest.fn() },
    });

    expect(() => adapter.create(type)).toThrow('no longer available');
    expect(getRuntime).not.toHaveBeenCalled();
  });
});

/** Build the catalog descriptor required by placement identity. */
function descriptor(): CubePickerDescriptor {
  return {
    key: 'a'.repeat(64),
    cubeId: 'local/demo.cube',
    version: '1.0.0',
    displayName: 'Demo Cube',
    description: '',
    searchTerms: [],
    targetModel: '',
    supportedModels: [],
    requiredCustomNodes: [],
    source: {},
    inputs: [],
    outputs: [],
  };
}
