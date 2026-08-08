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
/** Verify defensive host guards reject before graph and copied-definition mutation. */

import { describe, expect, jest, test } from '@jest/globals';
import { ComfyCubeHostMutationGuard } from '../../frontend/comfyui/ui/cube/placement/ComfyCubeHostMutationGuard.js';
import { CubeRootPlacementPolicy } from '../../frontend/comfyui/ui/cube/placement/CubeRootPlacementPolicy.js';

class GraphHost {
  readonly rootGraph: GraphHost;
  readonly added: unknown[] = [];

  constructor(root?: GraphHost) {
    this.rootGraph = root ?? this;
  }

  /** Record native adds so rejection ordering is observable. */
  add(node: unknown): unknown {
    this.added.push(node);
    return node;
  }
}

describe('ComfyCubeHostMutationGuard', () => {
  test('allows root Cubes and ordinary nested Subgraphs after hydration', () => {
    const root = new GraphHost();
    const nested = new GraphHost(root);
    const canvas = canvasHost(root);
    const reportError = jest.fn();
    const guard = new ComfyCubeHostMutationGuard({
      rootGraph: root,
      canvas,
      policy: new CubeRootPlacementPolicy(root),
      reportError,
    });
    guard.completeHydration();
    const cube = cubeCandidate();
    const ordinary = ordinarySubgraphCandidate();

    expect(root.add(cube)).toBe(cube);
    expect(nested.add(ordinary)).toBe(ordinary);
    expect(root.added).toEqual([cube]);
    expect(nested.added).toEqual([ordinary]);
    expect(reportError).not.toHaveBeenCalled();
    guard.dispose();
  });

  test('rejects a nested Cube before LGraph.add mutates membership', () => {
    const root = new GraphHost();
    const nested = new GraphHost(root);
    const canvas = canvasHost(nested);
    const reportError = jest.fn();
    const guard = new ComfyCubeHostMutationGuard({
      rootGraph: root,
      canvas,
      policy: new CubeRootPlacementPolicy(root),
      reportError,
    });
    guard.completeHydration();

    expect(() => nested.add(cubeCandidate())).toThrow('top-level workflow');
    expect(nested.added).toEqual([]);
    expect(reportError).toHaveBeenCalledWith(
      'SugarCube placement unavailable',
      expect.stringContaining('top-level workflow'),
    );
    guard.dispose();
  });

  test('allows historical nested Cubes only during workflow hydration', () => {
    const root = new GraphHost();
    const nested = new GraphHost(root);
    const guard = new ComfyCubeHostMutationGuard({
      rootGraph: root,
      canvas: canvasHost(root),
      policy: new CubeRootPlacementPolicy(root),
      reportError: jest.fn(),
    });
    const historical = cubeCandidate();

    expect(nested.add(historical)).toBe(historical);
    guard.completeHydration();
    expect(() => nested.add(cubeCandidate())).toThrow('top-level workflow');
    expect(nested.added).toEqual([historical]);
    guard.dispose();
  });

  test('preflights nested paste and clone before copied definitions are registered', () => {
    const root = new GraphHost();
    const nested = new GraphHost(root);
    const canvas = canvasHost(nested);
    const original = canvas._deserializeItems;
    const guard = new ComfyCubeHostMutationGuard({
      rootGraph: root,
      canvas,
      policy: new CubeRootPlacementPolicy(root),
      reportError: jest.fn(),
    });
    guard.completeHydration();
    const parsed = clipboardCube();

    expect(() => canvas._deserializeItems(parsed)).toThrow('no copied definitions');
    expect(canvas.deserialized).toEqual([]);
    guard.dispose();
    expect(canvas._deserializeItems).toBe(original);
  });

  test('allows root Cube copy but rejects a Cube buried in a copied ordinary Subgraph', () => {
    const root = new GraphHost();
    const canvas = canvasHost(root);
    const guard = new ComfyCubeHostMutationGuard({
      rootGraph: root,
      canvas,
      policy: new CubeRootPlacementPolicy(root),
      reportError: jest.fn(),
    });
    guard.completeHydration();
    const rootCube = clipboardCube();
    const invalidOrdinarySubgraph = {
      nodes: [{ id: 1, type: 'ordinary-definition', properties: {} }],
      subgraphs: [
        {
          id: 'ordinary-definition',
          extra: {},
          nodes: [{ id: 2, type: 'cube-definition', properties: {} }],
        },
        {
          id: 'cube-definition',
          extra: { sugarcubes_kind: 'cube' },
          nodes: [],
        },
      ],
    };

    expect(canvas._deserializeItems(rootCube)).toBe(rootCube);
    expect(() => canvas._deserializeItems(invalidOrdinarySubgraph)).toThrow(
      'nested inside a Subgraph',
    );
    expect(canvas.deserialized).toEqual([rootCube]);
    guard.dispose();
  });

  test('preserves ordinary Subgraph nesting when clipboard target is non-root', () => {
    const root = new GraphHost();
    const nested = new GraphHost(root);
    const canvas = canvasHost(nested);
    const guard = new ComfyCubeHostMutationGuard({
      rootGraph: root,
      canvas,
      policy: new CubeRootPlacementPolicy(root),
      reportError: jest.fn(),
    });
    guard.completeHydration();
    const ordinaryClipboard = {
      nodes: [{ id: 1, type: 'ordinary-definition', properties: {} }],
      subgraphs: [
        {
          id: 'ordinary-definition',
          extra: {},
          nodes: [{ id: 2, type: 'nested-ordinary-definition', properties: {} }],
        },
        { id: 'nested-ordinary-definition', extra: {}, nodes: [] },
      ],
    };

    expect(canvas._deserializeItems(ordinaryClipboard)).toBe(ordinaryClipboard);
    expect(canvas.deserialized).toEqual([ordinaryClipboard]);
    guard.dispose();
  });
});

interface CanvasHost {
  graph: GraphHost;
  deserialized: unknown[];
  _deserializeItems(parsed: unknown): unknown;
}

/** Build the stable canvas deserialization seam shared by paste and clone. */
function canvasHost(graph: GraphHost): CanvasHost {
  return {
    graph,
    deserialized: [],
    _deserializeItems(parsed: unknown) {
      this.deserialized.push(parsed);
      return parsed;
    },
  };
}

/** Build a configured native SubgraphNode carrying durable Cube identity. */
function cubeCandidate() {
  return {
    properties: { sugarcubes_kind: 'cube' },
    subgraph: { extra: { sugarcubes_kind: 'cube' } },
    isSubgraphNode: () => true,
  };
}

/** Build an ordinary SubgraphNode that must remain nestable. */
function ordinarySubgraphCandidate() {
  return {
    properties: {},
    subgraph: { extra: {} },
    isSubgraphNode: () => true,
  };
}

/** Build Comfy's serialized clipboard shape for one root Cube wrapper. */
function clipboardCube() {
  return {
    nodes: [{ id: 1, type: 'cube-definition', properties: { sugarcubes_kind: 'cube' } }],
    subgraphs: [
      {
        id: 'cube-definition',
        extra: { sugarcubes_kind: 'cube' },
        nodes: [{ id: 2, type: 'ordinary-node', properties: {} }],
      },
    ],
  };
}
