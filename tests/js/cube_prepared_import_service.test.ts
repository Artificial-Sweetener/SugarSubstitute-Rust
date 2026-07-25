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
/** Verify prepared-import orchestration stays outside the host entry point. */

import { jest } from '@jest/globals';
import { CubePreparedImportService } from '../../frontend/comfyui/ui/import/CubePreparedImportService.js';

describe('CubePreparedImportService', () => {
  test('registers native definitions and places one real Cube node', () => {
    const registerSubgraphs = jest.fn(() => ['registered']);
    const place = jest.fn(() => ({
      node: {
        id: 'surface-instance',
        pos: [40, 60] as [number, number],
        size: [900, 640] as [number, number],
      },
      warnings: ['placed'],
      internalNodeCount: 9,
    }));
    const service = new CubePreparedImportService({
      getGraph: () => ({}),
      getLiteGraph: () => ({ createNode() {}, vueNodesMode: true }),
      getNodeRenderer: () => 'vue',
      getRuntime: () => ({
        registerSubgraphs,
        placement: { place },
      }),
      readErrorMessage: (error) => String(error),
    });

    const result = service.apply(
      {
        cube: { cube_id: 'SDXL/Text to Image.cube', default_alias: 'SDXL/Text to Image' },
        nodes: [],
        layout: {
          origin: [40, 60],
          groups: [{ bounding: [0, 0, 900, 640], sugarcubes: { managed: true } }],
        },
      },
      { instanceAlias: 'Generation' },
    );

    expect(result).toMatchObject({
      success: true,
      summary: 'cube 1, internal nodes 9',
      primaryNodeId: 'surface-instance',
      warnings: ['registered', 'placed'],
      bounds: { minX: 40, minY: 60, maxX: 940, maxY: 700 },
      nodesAdded: 0,
      markersAdded: 0,
      connectionsMade: 0,
    });
    expect(registerSubgraphs).toHaveBeenCalledTimes(1);
    expect(place).toHaveBeenCalledWith(
      expect.objectContaining({ cube: expect.objectContaining({ cube_id: expect.any(String) }) }),
      { instanceAlias: 'Generation' },
    );
  });

  test.each([
    ['invalid payload', null, {}, { createNode() {} }, 'Importer payload missing'],
    ['missing graph', { cube: {} }, null, { createNode() {} }, 'Graph unavailable'],
    ['missing LiteGraph', { cube: {} }, {}, null, 'LiteGraph unavailable'],
    ['missing identity', {}, {}, { createNode() {} }, 'Importer payload is missing Cube identity'],
  ])('%s fails before runtime mutation', (_label, payload, graph, liteGraph, expected) => {
    const getRuntime = jest.fn(() => {
      throw new Error('Runtime must not be resolved.');
    });
    const service = new CubePreparedImportService({
      getGraph: () => graph,
      getLiteGraph: () => liteGraph,
      getNodeRenderer: () => undefined,
      getRuntime,
      readErrorMessage: (error) => String(error),
    });

    expect(service.apply(payload)).toMatchObject({ success: false, message: expected });
    expect(getRuntime).not.toHaveBeenCalled();
  });

  test('preserves placement failure context in the public result', () => {
    const service = new CubePreparedImportService({
      getGraph: () => ({}),
      getLiteGraph: () => ({ createNode() {} }),
      getNodeRenderer: () => 'litegraph',
      getRuntime: () => ({
        registerSubgraphs: () => [],
        placement: {
          place() {
            throw new Error('host rejected placement');
          },
        },
      }),
      readErrorMessage: (error) => (error instanceof Error ? error.message : String(error)),
    });

    expect(service.apply({ cube: { cube_id: 'example.cube' } })).toMatchObject({
      success: false,
      message: 'host rejected placement',
      warnings: ['Cube placement failed: host rejected placement'],
    });
  });
});
