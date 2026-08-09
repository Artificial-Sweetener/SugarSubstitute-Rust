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
import { ComfyCubeVersionReplacementAdapter } from '../../frontend/comfyui/ui/cube/version/ComfyCubeVersionReplacementAdapter.js';
import type { CubeVersionBoundaryMap } from '../../frontend/comfyui/ui/cube/version/CubeVersionBoundaryMap.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { ComfyLink, GraphId } from '../../frontend/comfyui/ui/types/graph.js';

const emptyMap = (): CubeVersionBoundaryMap => ({
  inputIdBySlot: new Map(),
  inputSlotById: new Map(),
  outputIdBySlot: new Map(),
  outputSlotById: new Map(),
});

function cube(version: string): CubeNode {
  return {
    id: 'instance',
    title: 'Demo',
    pos: [0, 0],
    size: [400, 300],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'instance', cube_version: version },
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: `definition-${version}`,
      name: 'Demo',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect: jest.fn(),
  };
}

describe('ComfyCubeVersionReplacementAdapter', () => {
  test('replaces one catalog entry inside one balanced history boundary', () => {
    const source = cube('1.0.0');
    const target = cube('2.0.0');
    const nodes = new Set<CubeNode>([source]);
    const catalog = new CubeNodeCatalog();
    catalog.add(source);
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const replaceSelection = jest.fn();
    const adapter = new ComfyCubeVersionReplacementAdapter({
      graph: {
        add: (node) => void nodes.add(node),
        remove: (node) => void nodes.delete(node),
        getNodeById: () => null,
        getLink: () => null,
      },
      catalog,
      history: { beforeChange, afterChange },
      selection: { replace: replaceSelection },
      logger: console,
    });

    adapter.replace({ source, target, sourceBoundaries: emptyMap(), targetBoundaries: emptyMap() });

    expect([...nodes]).toEqual([target]);
    expect(catalog.get('instance')).toBe(target);
    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    expect(replaceSelection).toHaveBeenCalledWith(source, target);
  });

  test('fails before mutation when a connected boundary is absent from the target', () => {
    const source = cube('1.0.0');
    source.inputs.push({ name: 'prompt', link: 7 });
    const target = cube('2.0.0');
    const catalog = new CubeNodeCatalog();
    catalog.add(source);
    const add = jest.fn();
    const remove = jest.fn();
    const links = new Map<GraphId, ComfyLink>([
      [7, { id: 7, origin_id: 'outside', origin_slot: 0, target_id: 'instance', target_slot: 0 }],
    ]);
    const adapter = new ComfyCubeVersionReplacementAdapter({
      graph: {
        add,
        remove,
        getNodeById: () => ({ connect: jest.fn() }),
        getLink: (id) => links.get(id) ?? null,
      },
      catalog,
      history: {},
      selection: { replace: jest.fn() },
      logger: console,
    });

    expect(() =>
      adapter.replace({
        source,
        target,
        sourceBoundaries: { ...emptyMap(), inputIdBySlot: new Map([[0, 'prompt-id']]) },
        targetBoundaries: emptyMap(),
      }),
    ).toThrow("cannot preserve connected input 'prompt'");
    expect(add).not.toHaveBeenCalled();
    expect(remove).not.toHaveBeenCalled();
    expect(catalog.get('instance')).toBe(source);
  });

  test('does not mutate catalog state when source removal fails', () => {
    const source = cube('1.0.0');
    const target = cube('2.0.0');
    const catalog = new CubeNodeCatalog();
    catalog.add(source);
    const add = jest.fn();
    const adapter = new ComfyCubeVersionReplacementAdapter({
      graph: {
        add,
        remove: () => {
          throw new Error('remove failed');
        },
        getNodeById: () => null,
        getLink: () => null,
      },
      catalog,
      history: {},
      selection: { replace: jest.fn() },
      logger: console,
    });

    expect(() =>
      adapter.replace({
        source,
        target,
        sourceBoundaries: emptyMap(),
        targetBoundaries: emptyMap(),
      }),
    ).toThrow('remove failed');
    expect(add).not.toHaveBeenCalled();
    expect(catalog.get('instance')).toBe(source);
  });
});
